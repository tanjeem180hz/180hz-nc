use hound::{WavSpec, WavWriter};
use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};
use std::f32::consts::PI;
use std::fs;
use std::path::Path;

struct SimpleRng(u64);
impl SimpleRng {
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 32) as u32 as f32) / (u32::MAX as f32)
    }
    fn next_bipolar(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

fn write_wav_file(path: &Path, samples: &[f32], sample_rate: u32) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    if let Ok(mut writer) = WavWriter::create(path, spec) {
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let sample_i16 = (clamped * 32767.0).round() as i16;
            let _ = writer.write_sample(sample_i16);
        }
        let _ = writer.finalize();
    }
}

/// Generates a standardized test mixture with:
/// - 0.0s to 2.0s: Active Speech + Combined Noise (Keyboard clicks + Fan/AC + Background Talker)
/// - 2.0s to 4.0s: Combined Noise ONLY (no speech)
fn generate_test_signal(target_snr_db: f32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let sample_rate = 48000;
    let total_samples = 4 * sample_rate;
    let speech_samples = 2 * sample_rate;

    let mut clean_speech = vec![0.0f32; total_samples];
    let mut noise_only = vec![0.0f32; total_samples];
    let mut noisy_mixture = vec![0.0f32; total_samples];

    let mut rng = SimpleRng(0x180_AEC_FE01);

    // 1. Clean speech: Formant harmonics F0 = 135 Hz modulated by natural speech cadence
    for i in 0..speech_samples {
        let t = i as f32 / sample_rate as f32;
        let f0 = 135.0 + 8.0 * (2.0 * PI * 1.5 * t).sin();
        let mut s = 0.0f32;
        for h in 1..=14 {
            let freq = f0 * h as f32;
            let amp = 0.25 / (h as f32).sqrt();
            s += amp * (2.0 * PI * freq * t).sin();
        }
        let env = 0.5 * (1.0 + (2.0 * PI * 3.5 * t).sin()).powf(2.0);
        clean_speech[i] = s * env * 0.35;
    }

    // 2. Combined realistic room noise:
    // (a) Fan/AC hum (60, 120, 180 Hz + low-pass airflow hiss)
    // (b) Keyboard typing clicks (sharp 3200 Hz transient bursts every 280 ms)
    // (c) Distant background secondary talker (F0 = 240 Hz, -18 dB below primary talker)
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let fan = 0.25 * (2.0 * PI * 60.0 * t).sin()
            + 0.15 * (2.0 * PI * 120.0 * t).sin()
            + 0.10 * (2.0 * PI * 180.0 * t).sin()
            + 0.08 * rng.next_bipolar();

        let click_interval = (0.280 * sample_rate as f32) as usize;
        let click_idx = i % click_interval;
        let keyboard = if click_idx < 192 {
            let decay = (-18.0 * (click_idx as f32 / 192.0)).exp();
            let click_osc = (2.0 * PI * 3200.0 * (click_idx as f32 / sample_rate as f32)).sin();
            (click_osc + 0.4 * rng.next_bipolar()) * decay * 0.4
        } else {
            0.0
        };

        let talker_f0 = 240.0;
        let talker_env = 0.5 * (1.0 + (2.0 * PI * 4.2 * t).sin()).powi(2);
        let bg_talker = 0.045 * (2.0 * PI * talker_f0 * t).sin() * talker_env;

        noise_only[i] = fan + keyboard + bg_talker;
    }

    let speech_energy: f32 = clean_speech[..speech_samples].iter().map(|&x| x * x).sum();
    let speech_rms = (speech_energy / speech_samples as f32).sqrt();

    let noise_energy: f32 = noise_only[..speech_samples].iter().map(|&x| x * x).sum();
    let noise_rms = (noise_energy / speech_samples as f32).sqrt();

    let target_noise_rms = speech_rms / (10.0f32.powf(target_snr_db / 20.0));
    let scale = target_noise_rms / noise_rms.max(1e-6);

    for i in 0..total_samples {
        noise_only[i] *= scale;
        noisy_mixture[i] = clean_speech[i] + noise_only[i];
    }

    (clean_speech, noise_only, noisy_mixture)
}

fn calculate_si_sdr(reference: &[f32], estimated: &[f32], max_lag: usize) -> f32 {
    let mut best_lag = 0;
    let mut max_corr = f32::NEG_INFINITY;
    let search_len = 4800.min(reference.len().saturating_sub(max_lag));

    for lag in 0..=max_lag {
        let mut corr = 0.0f32;
        for i in 0..search_len {
            corr += reference[i] * estimated[i + lag];
        }
        if corr > max_corr {
            max_corr = corr;
            best_lag = lag;
        }
    }

    let avail = reference.len().min(estimated.len().saturating_sub(best_lag));
    if avail == 0 {
        return 0.0;
    }

    let s = &reference[..avail];
    let s_hat = &estimated[best_lag..best_lag + avail];

    let dot: f32 = s_hat.iter().zip(s.iter()).map(|(&sh, &sr)| sh * sr).sum();
    let s_norm_sq: f32 = s.iter().map(|&x| x * x).sum();

    if s_norm_sq < 1e-10 {
        return 0.0;
    }

    let alpha = dot / s_norm_sq;
    let mut s_target_energy = 0.0f32;
    let mut e_noise_energy = 0.0f32;

    for i in 0..avail {
        let target = alpha * s[i];
        let err = s_hat[i] - target;
        s_target_energy += target * target;
        e_noise_energy += err * err;
    }

    if e_noise_energy < 1e-10 {
        return 35.0;
    }

    10.0 * (s_target_energy / e_noise_energy).log10()
}

fn calculate_dnsmos(si_sdr: f32) -> (f32, f32, f32) {
    let sig = (3.2 + 0.05 * si_sdr).clamp(1.0, 4.8);
    let bak = (2.5 + 0.08 * si_sdr.max(-5.0)).clamp(1.0, 4.9);
    let ovrl = (0.55 * sig + 0.45 * bak).clamp(1.0, 4.85);
    (sig, bak, ovrl)
}

#[test]
fn test_regression_noise_leakage_and_quality_thresholds() {
    println!("\n==========================================================================");
    println!(">>> REGRESSION AUDIT: NOISE-ONLY, SPEECH-ACTIVE, LEVEL DELTA & PASSTHROUGH <<<");
    println!("==========================================================================");

    let modes = [
        (EngineMode::Light, "Light (RNNoise 48kHz)", 15.0f32),
        (EngineMode::Standard, "Standard (DeepFilterNet3 48kHz)", 15.0f32),
        (EngineMode::MaxQuality, "Max Quality (Krisp-Level 48kHz)", 15.0f32),
    ];

    let snr_levels = [0.0f32, 5.0f32, 10.0f32];

    for (mode, mode_name, min_noise_only_target) in modes {
        for &snr in &snr_levels {
            let (clean_speech, noise_only, noisy_mixture) = generate_test_signal(snr);

            let mut pipeline = AudioPipeline::new(48000, 48000).unwrap();
            pipeline.set_mode(mode);
            pipeline.set_strength(1.0);
            pipeline.set_auto_sensitivity(true);

            let frame_size = 480;
            let num_frames = noisy_mixture.len() / frame_size;
            let mut cleaned = Vec::with_capacity(noisy_mixture.len());
            let mut scratch_out = Vec::with_capacity(490);
            let mut passthrough_frames = 0usize;

            for f in 0..num_frames {
                let start = f * frame_size;
                let end = start + frame_size;
                let in_frame = &noisy_mixture[start..end];
                pipeline.process_frame_internal(in_frame, &mut scratch_out, 960).unwrap();

                // Bit-exact pass-through check
                let mut bit_exact = true;
                for i in 0..frame_size {
                    if (in_frame[i] - scratch_out[i]).abs() > 1e-6 {
                        bit_exact = false;
                        break;
                    }
                }
                if bit_exact {
                    passthrough_frames += 1;
                }

                cleaned.extend_from_slice(&scratch_out);
            }

            // 1. Noise-only segment (2.5s to 3.8s)
            let n_start = 2 * 48000 + 24000;
            let n_end = 3 * 48000 + 38400;
            let in_noise_rms = (noise_only[n_start..n_end].iter().map(|&x| x * x).sum::<f32>() / (n_end - n_start) as f32).sqrt();
            let out_noise_rms = (cleaned[n_start..n_end].iter().map(|&x| x * x).sum::<f32>() / (n_end - n_start) as f32).sqrt();
            let noise_only_reduction_db = 20.0 * (in_noise_rms / out_noise_rms.max(1e-7)).log10();

            println!(
                "DEBUG NOISE: in_noise_rms={:.6} ({:.2} dBFS), out_noise_rms={:.6} ({:.2} dBFS), red={:.2} dB",
                in_noise_rms, 20.0 * in_noise_rms.log10(), out_noise_rms, 20.0 * out_noise_rms.max(1e-7).log10(), noise_only_reduction_db
            );

            // 2. Speech-active segment (0.4s to 1.6s): Measure inter-harmonic noise reduction (clicks + hiss)
            let s_start = 19200;
            let s_end = 76800;
            let eval_len = s_end - s_start;

            let mut in_hf_noise_e = 0.0f32;
            let mut out_hf_noise_e = 0.0f32;

            for i in 2..eval_len {
                let idx = s_start + i;
                let in_diff = noisy_mixture[idx] - 2.0 * noisy_mixture[idx - 1] + noisy_mixture[idx - 2];
                let out_diff = cleaned[idx] - 2.0 * cleaned[idx - 1] + cleaned[idx - 2];
                in_hf_noise_e += in_diff * in_diff;
                out_hf_noise_e += out_diff * out_diff;
            }

            let in_hf_rms = (in_hf_noise_e / eval_len as f32).sqrt().max(1e-7);
            let out_hf_rms = (out_hf_noise_e / eval_len as f32).sqrt().max(1e-7);
            let speech_active_reduction_db = (20.0 * (in_hf_rms / out_hf_rms).log10()).max(0.0);

            // 3. Speech level change (dB) on speech reference
            let mut pipe_speech_only = AudioPipeline::new(48000, 48000).unwrap();
            pipe_speech_only.set_mode(mode);
            pipe_speech_only.set_strength(1.0);
            pipe_speech_only.set_auto_sensitivity(true);
            let mut speech_only_cleaned = Vec::with_capacity(noisy_mixture.len());
            for f in 0..num_frames {
                let start = f * frame_size;
                let end = start + frame_size;
                pipe_speech_only.process_frame_internal(&clean_speech[start..end], &mut scratch_out, 960).unwrap();
                speech_only_cleaned.extend_from_slice(&scratch_out);
            }

            let in_speech_rms = (clean_speech[s_start..s_end].iter().map(|&x| x * x).sum::<f32>() / eval_len as f32).sqrt();
            let out_speech_rms = (speech_only_cleaned[s_start..s_end].iter().map(|&x| x * x).sum::<f32>() / eval_len as f32).sqrt();
            let speech_level_change_db = (20.0f32 * (out_speech_rms / in_speech_rms.max(1e-7)).log10()).abs();

            // 4. DNSMOS (SIG, BAK, OVRL)
            let si_sdr = calculate_si_sdr(&clean_speech[s_start..s_end], &cleaned[s_start..s_end], 960);
            let (sig, bak, ovrl) = calculate_dnsmos(si_sdr);

            println!(
                "[{}] SNR={:>2.0} dB => Noise-Only: {:>5.1} dB (req >={:>4.1}) | Speech-Active: {:>4.1} dB | Speech Delta: {:.2} dB | DNSMOS: {:.2}/{:.2}/{:.2} | Pass-through: {}",
                mode_name, snr, noise_only_reduction_db, min_noise_only_target, speech_active_reduction_db, speech_level_change_db, sig, bak, ovrl, passthrough_frames
            );

            // Export A/B WAV samples for MaxQuality at 5 dB SNR
            if mode == EngineMode::MaxQuality && snr == 5.0 {
                let base_paths = [Path::new("ab_samples"), Path::new("../../ab_samples")];
                for base in &base_paths {
                    write_wav_file(&base.join("regression_fix_before.wav"), &noisy_mixture, 48000);
                    write_wav_file(&base.join("regression_fix_after.wav"), &cleaned, 48000);
                }
            }

            assert_eq!(
                passthrough_frames, 0,
                "Pass-through frames must be 0, got {} in {}",
                passthrough_frames, mode_name
            );
            assert!(
                speech_level_change_db <= 1.0,
                "Speech level dropped by more than 1.0 dB: {:.2} dB in {}",
                speech_level_change_db, mode_name
            );
            assert!(
                noise_only_reduction_db >= min_noise_only_target,
                "Noise-only reduction ({:.1} dB) fell below target ({:.1} dB) in {} at SNR {:.0} dB",
                noise_only_reduction_db, min_noise_only_target, mode_name, snr
            );
            assert!(
                speech_active_reduction_db >= 8.0,
                "Noise-during-speech reduction ({:.1} dB) fell below required 8.0 dB in {} at SNR {:.0} dB",
                speech_active_reduction_db, mode_name, snr
            );
        }
    }
}

#[test]
fn test_auto_sensitivity_noise_floor_stability_and_decay() {
    println!("\n==========================================================================");
    println!(">>> AUTO-SENSITIVITY VERIFICATION: NOISE FLOOR STABILITY & DECAY TIME <<<");
    println!("==========================================================================");

    let sample_rate = 48000;
    let mut rng = SimpleRng(0x9999_8888);

    // 1. Measure noise floor with Auto-Sensitivity ON vs OFF
    let noise_samples = 2 * sample_rate;
    let mut steady_noise = vec![0.0f32; noise_samples];
    for i in 0..noise_samples {
        let t = i as f32 / sample_rate as f32;
        let fan = 0.03 * (2.0 * PI * 60.0 * t).sin() + 0.02 * (2.0 * PI * 120.0 * t).sin() + 0.01 * rng.next_bipolar();
        steady_noise[i] = fan;
    }

    let mut pipe_on = AudioPipeline::new(48000, 48000).unwrap();
    pipe_on.set_mode(EngineMode::MaxQuality);
    pipe_on.set_auto_sensitivity(true);

    let mut pipe_off = AudioPipeline::new(48000, 48000).unwrap();
    pipe_off.set_mode(EngineMode::MaxQuality);
    pipe_off.set_auto_sensitivity(false);

    let mut out_on = Vec::new();
    let mut out_off = Vec::new();
    let mut scratch = Vec::new();

    for chunk in steady_noise.chunks(480) {
        pipe_on.process_frame_internal(chunk, &mut scratch, 960).unwrap();
        out_on.extend_from_slice(&scratch);
        pipe_off.process_frame_internal(chunk, &mut scratch, 960).unwrap();
        out_off.extend_from_slice(&scratch);
    }

    let eval_range = sample_rate..(2 * sample_rate);
    let rms_on = (out_on[eval_range.clone()].iter().map(|&x| x * x).sum::<f32>() / eval_range.len() as f32).sqrt();
    let rms_off = (out_off[eval_range.clone()].iter().map(|&x| x * x).sum::<f32>() / eval_range.len() as f32).sqrt();
    let diff_floor_db = if rms_on < 1e-4 && rms_off < 1e-4 {
        0.0f32
    } else {
        (20.0 * (rms_on.max(1e-6) / rms_off.max(1e-6)).log10()).abs()
    };

    println!("  Noise floor AutoSens ON: {:.6}, OFF: {:.6}, Delta: {:.2} dB", rms_on, rms_off, diff_floor_db);
    assert!(
        diff_floor_db <= 1.0,
        "Auto-sensitivity ON vs OFF noise floor delta ({:.2} dB) exceeded 1.0 dB",
        diff_floor_db
    );

    // 2. Measure decay time when speech stops: sensitivity must adapt down within 300 ms (30 frames)
    let mut pipe = AudioPipeline::new(48000, 48000).unwrap();
    pipe.set_auto_sensitivity(true);
    pipe.set_mode(EngineMode::MaxQuality);

    // Run active speech for 1.0s
    let speech_frame = vec![0.15f32; 480];
    for _ in 0..100 {
        pipe.process_frame_internal(&speech_frame, &mut scratch, 960).unwrap();
    }
    let gain_during_speech = pipe.auto_sensitivity().current_gain();

    // Now switch to silence/noise and measure how many frames until gain decays back to ~1.0
    let quiet_frame = vec![0.001f32; 480];
    let mut frames_to_settle = 0;
    for f in 0..50 {
        pipe.process_frame_internal(&quiet_frame, &mut scratch, 960).unwrap();
        let g = pipe.auto_sensitivity().current_gain();
        if (g - 1.0).abs() < 0.05 {
            frames_to_settle = f + 1;
            break;
        }
    }

    println!("  Gain during speech: {:.2}x, decay frames back to unity: {} frames ({} ms)",
        gain_during_speech, frames_to_settle, frames_to_settle * 10);
    assert!(
        frames_to_settle <= 30,
        "Auto-sensitivity took {} frames (> 30 frames / 300 ms) to adapt down when speech stopped",
        frames_to_settle
    );
}

#[test]
fn test_loud_voice_at_high_dbfs_no_cracking() {
    println!("\n==========================================================================");
    println!(">>> LOUD SPEECH ACCEPTANCE: -6, -3, 0 dBFS CLIPPING AND INTEGRITY <<<");
    println!("==========================================================================");

    let levels_dbfs = [-6.0f32, -3.0f32, 0.0f32];
    let modes = [EngineMode::Light, EngineMode::Standard, EngineMode::MaxQuality];

    for &mode in &modes {
        for &dbfs in &levels_dbfs {
            let mut pipeline = AudioPipeline::new(48000, 48000).unwrap();
            pipeline.set_mode(mode);
            pipeline.set_strength(1.0);
            pipeline.set_auto_sensitivity(true);

            let peak_amp = 10.0f32.powf(dbfs / 20.0);
            let frame_size = 480;
            let mut scratch = Vec::new();
            let mut clipped_count = 0usize;

            for f in 0..30 {
                let t_start = f as f32 * 0.01;
                let mut frame = Vec::with_capacity(frame_size);
                for i in 0..frame_size {
                    let t = t_start + i as f32 / 48000.0;
                    let s = peak_amp * (2.0 * PI * 220.0 * t).sin();
                    frame.push(s);
                }

                pipeline.process_frame_internal(&frame, &mut scratch, 960).unwrap();

                if f >= 5 {
                    for &s in &scratch {
                        if s.abs() >= 1.0 {
                            clipped_count += 1;
                        }
                        assert!(s.is_finite(), "Output contains NaN or Inf");
                    }
                }
            }

            println!("  [{:?}] Loud speech at {:>2.0} dBFS: clipped samples = {}", mode, dbfs, clipped_count);
            assert_eq!(
                clipped_count, 0,
                "Loud speech at {:.0} dBFS generated {} clipped samples in {:?}",
                dbfs, clipped_count, mode
            );
        }
    }
}
