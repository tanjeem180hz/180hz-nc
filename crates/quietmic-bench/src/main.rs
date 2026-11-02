mod metrics;
mod test_set;

use hound::{WavSpec, WavWriter};
use metrics::{AudioMetricsEvaluator, BenchmarkResult};
use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};
use std::fs;
use std::path::Path;
use std::time::Instant;
use test_set::TestSetGenerator;

fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create wav writer at {:?}: {}", path, e))?;

    for &s in samples {
        // Clamp and convert to 16-bit PCM
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0) as i16;
        writer.write_sample(sample_i16)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer.finalize().map_err(|e| format!("Failed to finalize wav: {}", e))?;
    Ok(())
}

fn benchmark_pipeline(
    scenario_name: &str,
    mode: EngineMode,
    clean: &[f32],
    noisy: &[f32],
    sample_rate: u32,
    output_wav_path: Option<&Path>,
) -> BenchmarkResult {
    let mut pipeline = AudioPipeline::new(sample_rate, sample_rate).expect("Pipeline init failed");
    pipeline.set_mode(mode);
    pipeline.set_strength(1.0);

    // If VoiceIsolation mode, enroll using first 1 second of clean speech
    if mode == EngineMode::VoiceIsolation {
        let _ = pipeline.voice_isolation_mut().enroll(&clean[..48000.min(clean.len())]);
    }

    let mut cleaned_audio = Vec::with_capacity(noisy.len());
    let mut frame_times_us = Vec::new();
    let mut scratch_out = Vec::new();

    let frame_size = 480;
    let mut offset = 0;

    let t_start_total = Instant::now();

    while offset + frame_size <= noisy.len() {
        let in_frame = &noisy[offset..offset + frame_size];

        let t0 = Instant::now();
        let _ = pipeline.process_frame_internal(in_frame, &mut scratch_out, 960);
        let elapsed_us = t0.elapsed().as_micros() as f32;

        frame_times_us.push(elapsed_us);
        cleaned_audio.extend_from_slice(&scratch_out);

        offset += frame_size;
    }

    let total_audio_duration_s = offset as f32 / sample_rate as f32;
    let total_proc_time_s = t_start_total.elapsed().as_secs_f32();
    let rtf = total_proc_time_s / total_audio_duration_s.max(0.001);

    frame_times_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean_us = frame_times_us.iter().sum::<f32>() / frame_times_us.len().max(1) as f32;
    let p99_idx = ((frame_times_us.len() as f32) * 0.99) as usize;
    let p99_us = frame_times_us.get(p99_idx).copied().unwrap_or(mean_us);
    let max_us = frame_times_us.last().copied().unwrap_or(mean_us);

    let (in_snr, out_snr, delta_snr) =
        AudioMetricsEvaluator::calculate_snr_improvement(clean, noisy, &cleaned_audio);
    let si_sdr = AudioMetricsEvaluator::calculate_si_sdr(clean, &cleaned_audio);
    let (sig, bak, ovrl) = AudioMetricsEvaluator::evaluate_dnsmos(clean, &cleaned_audio);

    if let Some(path) = output_wav_path {
        let _ = write_wav(path, &cleaned_audio, sample_rate);
    }

    let mode_str = match mode {
        EngineMode::Light => "Light (RNNoise)",
        EngineMode::Standard => "Standard (DeepFilterNet3)",
        EngineMode::VoiceIsolation => "Voice Isolation",
    };

    BenchmarkResult {
        scenario: scenario_name.to_string(),
        mode: mode_str.to_string(),
        input_snr_db: in_snr,
        output_snr_db: out_snr,
        snr_improvement_db: delta_snr,
        si_sdr_db: si_sdr,
        dnsmos_sig: sig,
        dnsmos_bak: bak,
        dnsmos_ovrl: ovrl,
        mean_frame_us: mean_us,
        p99_frame_us: p99_us,
        max_frame_us: max_us,
        real_time_factor: rtf,
    }
}

fn main() {
    println!("================================================================================");
    println!(" QuietMic DSP & ML Verification Harness");
    println!(" Benchmarking Light, Standard, and Voice Isolation modes");
    println!("================================================================================");

    let output_dir = Path::new("target/bench_output");
    let _ = fs::create_dir_all(output_dir);

    let scenarios = TestSetGenerator::create_test_suite();
    println!("Loaded {} test scenarios across 4 noise profiles & 4 SNR levels (-5dB to 10dB).", scenarios.len());

    let mut all_results = Vec::new();

    // Representative scenarios for detailed table
    let key_scenarios = [
        "Keyboard_Clicks_+0dB_SNR",
        "Fan_AC_Hum_+0dB_SNR",
        "Cafe_Babble_+5dB_SNR",
        "Background_Talker_+5dB_SNR",
    ];

    println!("\n{:<28} | {:<24} | {:>8} | {:>8} | {:>6} | {:>6} | {:>6} | {:>8} | {:>6}",
        "Scenario", "Mode", "In SNR", "Out SNR", "SIG", "BAK", "OVRL", "Avg (us)", "RTF");
    println!("{:-<125}", "");

    for sc in &scenarios {
        let is_key = key_scenarios.iter().any(|&k| sc.name == k);

        // Save original noisy mixture for key scenarios
        if is_key {
            let orig_path = output_dir.join(format!("original_{}.wav", sc.name));
            let _ = write_wav(&orig_path, &sc.noisy_mixture, sc.sample_rate);
        }

        // Test Unprocessed Baseline
        let _baseline_si_sdr = AudioMetricsEvaluator::calculate_si_sdr(&sc.clean_speech, &sc.noisy_mixture);
        let (base_sig, base_bak, base_ovrl) = AudioMetricsEvaluator::evaluate_dnsmos(&sc.clean_speech, &sc.noisy_mixture);

        if is_key {
            println!("{:<28} | {:<24} | {:>7.1}dB | {:>7.1}dB | {:>6.2} | {:>6.2} | {:>6.2} | {:>8} | {:>6}",
                sc.name, "Unprocessed (Raw)", sc.target_snr_db, sc.target_snr_db, base_sig, base_bak, base_ovrl, "-", "-");
        }

        // Test Light Engine (RNNoise)
        let light_wav = if is_key { Some(output_dir.join(format!("cleaned_light_{}.wav", sc.name))) } else { None };
        let res_light = benchmark_pipeline(&sc.name, EngineMode::Light, &sc.clean_speech, &sc.noisy_mixture, sc.sample_rate, light_wav.as_deref());
        if is_key {
            println!("{:<28} | {:<24} | {:>7.1}dB | {:>7.1}dB | {:>6.2} | {:>6.2} | {:>6.2} | {:>7.1}us | {:>6.4}",
                sc.name, res_light.mode, res_light.input_snr_db, res_light.output_snr_db,
                res_light.dnsmos_sig, res_light.dnsmos_bak, res_light.dnsmos_ovrl,
                res_light.mean_frame_us, res_light.real_time_factor);
        }
        all_results.push(res_light);

        // Test Standard Engine (DeepFilterNet3)
        let std_wav = if is_key { Some(output_dir.join(format!("cleaned_standard_{}.wav", sc.name))) } else { None };
        let res_std = benchmark_pipeline(&sc.name, EngineMode::Standard, &sc.clean_speech, &sc.noisy_mixture, sc.sample_rate, std_wav.as_deref());
        if is_key {
            println!("{:<28} | {:<24} | {:>7.1}dB | {:>7.1}dB | {:>6.2} | {:>6.2} | {:>6.2} | {:>7.1}us | {:>6.4}",
                sc.name, res_std.mode, res_std.input_snr_db, res_std.output_snr_db,
                res_std.dnsmos_sig, res_std.dnsmos_bak, res_std.dnsmos_ovrl,
                res_std.mean_frame_us, res_std.real_time_factor);
        }
        all_results.push(res_std);

        // Test Voice Isolation Engine for background talker
        if sc.name.contains("Background_Talker") {
            let iso_wav = if is_key { Some(output_dir.join(format!("cleaned_voice_iso_{}.wav", sc.name))) } else { None };
            let res_iso = benchmark_pipeline(&sc.name, EngineMode::VoiceIsolation, &sc.clean_speech, &sc.noisy_mixture, sc.sample_rate, iso_wav.as_deref());
            if is_key {
                println!("{:<28} | {:<24} | {:>7.1}dB | {:>7.1}dB | {:>6.2} | {:>6.2} | {:>6.2} | {:>7.1}us | {:>6.4}",
                    sc.name, res_iso.mode, res_iso.input_snr_db, res_iso.output_snr_db,
                    res_iso.dnsmos_sig, res_iso.dnsmos_bak, res_iso.dnsmos_ovrl,
                    res_iso.mean_frame_us, res_iso.real_time_factor);
            }
            all_results.push(res_iso);
        }
    }

    println!("{:-<125}", "");

    // Export full JSON report
    let report_path = output_dir.join("benchmark_report.json");
    let json_str = serde_json::to_string_pretty(&all_results).expect("JSON serialization failed");
    let _ = fs::write(&report_path, json_str);
    println!("\nFull benchmark report written to: {:?}", report_path);
    println!("A/B comparison WAV files saved to: {:?}", output_dir);
}
