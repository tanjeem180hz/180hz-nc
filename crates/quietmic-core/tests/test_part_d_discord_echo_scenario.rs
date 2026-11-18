use quietmic_core::audio::echo_doctor::{EchoDoctor, EndpointType};
use quietmic_core::engine::aec::EchoCanceller;
use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};
use std::f32::consts::PI;
use std::path::Path;

fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut writer) = hound::WavWriter::create(path, spec) {
        for &s in samples {
            let clamped = s.clamp(-1.0, 1.0);
            let val = (clamped * 32767.0) as i16;
            let _ = writer.write_sample(val);
        }
        let _ = writer.finalize();
    }
}

fn synthesize_voice(t_start: f32, num_samples: usize, f0: f32, amp: f32) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = t_start + (i as f32 / 48000.0);
        let v1 = (2.0 * PI * f0 * t).sin() * 0.50;
        let v2 = (2.0 * PI * (f0 * 2.0) * t).sin() * 0.30;
        let v3 = (2.0 * PI * (f0 * 3.0) * t).sin() * 0.15;
        out.push((v1 + v2 + v3) * amp);
    }
    out
}

#[test]
fn test_discord_screen_share_scenario_headphones_vs_speakers() {
    println!("\n=======================================================");
    println!(">>> RUNNING DISCORD SCREEN SHARE & AEC SCENARIO TEST <<<");
    println!("=======================================================");

    const FRAME_SIZE: usize = 480;
    const NUM_FRAMES: usize = 300; // 3 seconds

    // --- Scenario A: Headphones Detected (Auto-Bypass & Zero Bleed) ---
    {
        let endpoint = EchoDoctor::detect_endpoint_type("Realtek HD Audio (Sennheiser HD650 Headphones)");
        assert_eq!(endpoint, EndpointType::Headphones);
        assert!(!endpoint.aec_recommended(), "Headphones should not require AEC");

        let mut pipeline = AudioPipeline::new(48000, 48000).unwrap();
        pipeline.set_mode(EngineMode::MaxQuality);
        pipeline.set_strength(1.0);
        pipeline.set_aec_enabled(false); // Auto-bypassed

        let mut raw_mic = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
        let mut cleaned_out = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
        let mut out_chunk = Vec::with_capacity(FRAME_SIZE + 10);

        for f in 0..NUM_FRAMES {
            let t = f as f32 * 0.01;
            // Near-end speaker talking while wearing headphones
            let speech = synthesize_voice(t, FRAME_SIZE, 130.0, 0.35);
            let mic_frame = speech.clone();

            raw_mic.extend_from_slice(&mic_frame);
            pipeline.process_frame_internal(&mic_frame, &mut out_chunk, 0).unwrap();
            cleaned_out.extend_from_slice(&out_chunk);
        }

        let raw_rms = (raw_mic.iter().map(|&x| x * x).sum::<f32>() / raw_mic.len() as f32).sqrt();
        let cleaned_rms = (cleaned_out.iter().map(|&x| x * x).sum::<f32>() / cleaned_out.len() as f32).sqrt();
        let diff_db = (cleaned_rms / (raw_rms + 1e-6)).log10() * 20.0;
        println!("Scenario A (Headphones): Raw RMS: {:.4}, Cleaned RMS: {:.4}, Level Delta: {:.2} dB", raw_rms, cleaned_rms, diff_db);

        assert!(diff_db.abs() < 1.0, "Speech must be preserved transparently through headphones: delta = {:.2} dB", diff_db);

        // Export WAVs
        let base_dirs = [Path::new("ab_samples"), Path::new("../../ab_samples")];
        for base in &base_dirs {
            write_wav(&base.join("discord_headphones_raw.wav"), &raw_mic, 48000);
            write_wav(&base.join("discord_headphones_cleaned.wav"), &cleaned_out, 48000);
        }
    }

    // --- Scenario B: Loudspeakers with Far-End Discord Call Audio (AEC Active) ---
    {
        let endpoint = EchoDoctor::detect_endpoint_type("Realtek High Definition Audio Speakers");
        assert_eq!(endpoint, EndpointType::Speakers);
        assert!(endpoint.aec_recommended(), "Speakers should recommend AEC");

        let mut canceller = EchoCanceller::default_preset();
        canceller.set_enabled(true);
        canceller.set_bulk_delay_samples(0);

        let mut raw_echo = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
        let mut cleaned_echo = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);

        for f in 0..NUM_FRAMES {
            let t = f as f32 * 0.01;
            // Far-end caller talking in Discord (e.g. 210 Hz pitch)
            let far_end = synthesize_voice(t, FRAME_SIZE, 210.0, 0.40);

            // Loudspeaker acoustic bleed into room mic: direct path 0.35 + reflection
            let mut mic_input = vec![0.0f32; FRAME_SIZE];
            for i in 0..FRAME_SIZE {
                mic_input[i] = far_end[i] * 0.35;
            }

            raw_echo.extend_from_slice(&mic_input);

            canceller.feed_reference(&far_end);
            canceller.process_mic_frame(&mut mic_input);

            cleaned_echo.extend_from_slice(&mic_input);
        }

        let raw_rms = (raw_echo.iter().map(|&x| x * x).sum::<f32>() / raw_echo.len() as f32).sqrt();
        let cleaned_rms = (cleaned_echo.iter().map(|&x| x * x).sum::<f32>() / cleaned_echo.len() as f32).sqrt();
        let erle_db = 20.0 * (raw_rms / (cleaned_rms + 1e-6)).log10();
        println!("Scenario B (Loudspeakers): Raw Echo RMS: {:.4}, Cleaned Echo RMS: {:.5}, ERLE: {:.2} dB", raw_rms, cleaned_rms, erle_db);

        assert!(erle_db >= 20.0, "AEC must eliminate speaker echo with at least 20 dB ERLE, got {:.2} dB", erle_db);

        // Export WAVs
        let base_dirs = [Path::new("ab_samples"), Path::new("../../ab_samples")];
        for base in &base_dirs {
            write_wav(&base.join("discord_screen_share_raw.wav"), &raw_echo, 48000);
            write_wav(&base.join("discord_screen_share_cleaned.wav"), &cleaned_echo, 48000);
        }
    }

    // --- Scenario C: Misconfigured Default Playback Device (Virtual Cable) ---
    {
        let report = EchoDoctor::diagnose();
        // The diagnostic structure should always contain Discord tips and step-by-step guidance
        assert!(!report.discord_screen_share_tips.is_empty());
        assert!(!report.plain_explanation.is_empty());
    }
}
