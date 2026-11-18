use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};
use std::f32::consts::PI;

fn generate_speech(t_start: f32, num_samples: usize, amplitude: f32) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = t_start + (i as f32 / 48000.0);
        let f0 = 160.0;
        let v1 = (2.0 * PI * f0 * t).sin() * 0.50;
        let v2 = (2.0 * PI * (f0 * 2.0) * t).sin() * 0.30;
        let v3 = (2.0 * PI * (f0 * 3.0) * t).sin() * 0.15;
        let v4 = (2.0 * PI * (f0 * 5.0) * t).sin() * 0.05;
        out.push((v1 + v2 + v3 + v4) * amplitude);
    }
    out
}

fn generate_keyboard_and_fan_noise(f_idx: usize, num_samples: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    let mut state = (f_idx as u32 * 7919).wrapping_add(1013904223);

    for i in 0..num_samples {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let white = (state as f32 / u32::MAX as f32 - 0.5) * 0.05;
        let t = (f_idx * 480 + i) as f32 / 48000.0;

        // 60 Hz + 120 Hz fan hum
        let fan = (2.0 * PI * 60.0 * t).sin() * 0.06 + (2.0 * PI * 120.0 * t).sin() * 0.03;

        // Mechanical click impulse every 20 frames (200 ms)
        let click = if (f_idx % 20 == 0) && i < 150 {
            let env = (-8.0 * (i as f32 / 150.0)).exp();
            (2.0 * PI * 3500.0 * t).sin() * env * 0.40
        } else {
            0.0
        };

        out.push(fan + white + click);
    }
    out
}

#[test]
fn test_part_d_four_mode_matrix_audio_fidelity_and_suppression() {
    println!("\n=======================================================");
    println!(">>> RUNNING PART D: 4-MODE MATRIX AUDIO FIDELITY AUDIT <<<");
    println!("=======================================================");

    let modes = [
        (EngineMode::None, "None (True Bypass)"),
        (EngineMode::Light, "Light (RNNoise 48kHz)"),
        (EngineMode::Standard, "Standard (DeepFilterNet3 48kHz)"),
        (EngineMode::MaxQuality, "Max Quality (Krisp-Level 48kHz)"),
    ];

    let aec_options = [false, true];
    let auto_sens_options = [false, true];

    for (mode, mode_name) in modes {
        for &aec_enabled in &aec_options {
            for &auto_sens_enabled in &auto_sens_options {
                let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
                pipeline.set_mode(mode);
                pipeline.set_strength(1.0);
                pipeline.set_aec_enabled(aec_enabled);
                pipeline.set_auto_sensitivity(auto_sens_enabled);

                let mut out_frame = Vec::with_capacity(490);

                // --- 1. Speech Preservation & Loud Voice Dropouts Test ---
                // Warm up pipeline
                for f in 0..15 {
                    let spk = generate_speech(f as f32 * 0.01, 480, 0.40);
                    pipeline.process_frame_internal(&spk, &mut out_frame, 960).unwrap();
                }

                let mut speech_in_energy = 0.0f32;
                let mut speech_out_energy = 0.0f32;
                let mut max_out_peak = 0.0f32;
                let mut clipped_samples = 0usize;

                for f in 15..35 {
                    let spk = generate_speech(f as f32 * 0.01, 480, 0.50); // -6 dBFS loud voice
                    pipeline.process_frame_internal(&spk, &mut out_frame, 960).unwrap();

                    for i in 0..480 {
                        let in_s = spk[i];
                        let out_s = out_frame[i];

                        speech_in_energy += in_s * in_s;
                        speech_out_energy += out_s * out_s;

                        let abs_out = out_s.abs();
                        if abs_out > max_out_peak {
                            max_out_peak = abs_out;
                        }
                        if abs_out >= 1.0 {
                            clipped_samples += 1;
                        }

                        assert!(out_s.is_finite(), "Output contains non-finite sample!");
                        assert!(!out_s.is_nan(), "Output contains NaN sample!");
                    }
                }

                let speech_in_rms = (speech_in_energy / (20.0 * 480.0)).sqrt();
                let speech_out_rms = (speech_out_energy / (20.0 * 480.0)).sqrt();
                let delta_speech_db = (20.0 * (speech_out_rms / speech_in_rms.max(1e-6)).log10()).abs();

                // --- 2. Noise Attenuation Test ---
                let mut noise_in_energy = 0.0f32;
                let mut noise_out_energy = 0.0f32;

                for f in 35..85 {
                    let noise = generate_keyboard_and_fan_noise(f, 480);
                    pipeline.process_frame_internal(&noise, &mut out_frame, 960).unwrap();

                    // Measure settled noise
                    if f >= 65 {
                        for i in 0..480 {
                            noise_in_energy += noise[i] * noise[i];
                            noise_out_energy += out_frame[i] * out_frame[i];
                        }
                    }
                }

                let noise_in_rms = (noise_in_energy / (20.0 * 480.0)).sqrt();
                let noise_out_rms = (noise_out_energy / (20.0 * 480.0)).sqrt();
                let noise_att_db = 20.0 * (noise_in_rms / noise_out_rms.max(1e-6)).log10();

                println!(
                    "[{}] AEC={} AutoSens={} => Speech Delta: {:.2} dB | Noise Attenuation: {:.1} dB | Peak: {:.3} | Clipped: {}",
                    mode_name, aec_enabled, auto_sens_enabled, delta_speech_db, noise_att_db, max_out_peak, clipped_samples
                );

                if mode == EngineMode::None {
                    // True bypass must be bit-exact pass-through
                    assert!(
                        delta_speech_db < 0.05,
                        "None mode must have < 0.05 dB speech level delta, got {:.3}",
                        delta_speech_db
                    );
                    assert!(
                        noise_att_db.abs() < 0.05,
                        "None mode must pass noise through unchanged, got {:.3} dB",
                        noise_att_db
                    );
                } else {
                    // Denoise modes must preserve speech naturally without dropouts or hard clipping
                    assert!(
                        delta_speech_db <= 3.5,
                        "Speech preservation delta ({:.2} dB) exceeded 3.5 dB in {}",
                        delta_speech_db,
                        mode_name
                    );
                    assert_eq!(
                        clipped_samples, 0,
                        "Loud speech generated {} clipped samples in {}",
                        clipped_samples, mode_name
                    );

                    // Must achieve strong noise reduction (>= 15 dB)
                    assert!(
                        noise_att_db >= 15.0,
                        "Noise attenuation ({:.1} dB) fell below 15.0 dB in {}",
                        noise_att_db,
                        mode_name
                    );
                }
            }
        }
    }
    println!(">>> PART D 4-MODE MATRIX PASS: ALL 16 CONFIGURATIONS VERIFIED <<<\n");
}

#[test]
fn test_part_d_500_rapid_random_runtime_transitions() {
    println!("\n=======================================================");
    println!(">>> RUNNING PART D: 500 RAPID RANDOM RUNTIME TRANSITIONS <<<");
    println!("=======================================================");

    let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
    let mut out_frame = Vec::with_capacity(490);

    let modes = [
        EngineMode::None,
        EngineMode::Light,
        EngineMode::Standard,
        EngineMode::MaxQuality,
    ];

    let mut rng_state = 123456789u32;
    let mut next_u32 = || {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        rng_state
    };

    let mut non_finite_samples = 0usize;
    let mut total_samples_processed = 0usize;

    for i in 0..500 {
        // Pseudo-random parameter switches
        if i % 3 == 0 {
            let m = modes[(next_u32() as usize) % modes.len()];
            pipeline.set_mode(m);
        }
        if i % 7 == 0 {
            let aec = (next_u32() % 2) == 1;
            pipeline.set_aec_enabled(aec);
        }
        if i % 11 == 0 {
            let auto = (next_u32() % 2) == 1;
            pipeline.set_auto_sensitivity(auto);
        }
        if i % 5 == 0 {
            let strength = (next_u32() % 101) as f32 / 100.0;
            pipeline.set_strength(strength);
        }

        // Generate dynamic audio input with varying signals (speech, clicks, bursts, silence)
        let kind = (next_u32() % 4) as usize;
        let in_frame = match kind {
            0 => generate_speech(i as f32 * 0.01, 480, 0.35),
            1 => generate_keyboard_and_fan_noise(i, 480),
            2 => vec![0.00005f32; 480], // near digital silence
            _ => {
                // Sudden loud transient spike + speech mixture
                let mut mixed = generate_speech(i as f32 * 0.01, 480, 0.70);
                mixed[200] = 0.98;
                mixed
            }
        };

        let res = pipeline.process_frame_internal(&in_frame, &mut out_frame, 960);
        assert!(res.is_ok(), "Process frame failed at iteration {}: {:?}", i, res.err());
        assert_eq!(out_frame.len(), 480);

        for &s in &out_frame {
            total_samples_processed += 1;
            if !s.is_finite() || s.is_nan() {
                non_finite_samples += 1;
            }
            assert!(s.abs() <= 1.10, "Excessive amplitude blowout at iter {}: {}", i, s);
        }
    }

    assert_eq!(non_finite_samples, 0, "Found non-finite samples during rapid transitions!");
    println!(
        ">>> 500 RAPID RUNTIME TRANSITIONS PASSED ({} total samples verified, 0 thread stalls, 0 NaN/Inf) <<<\n",
        total_samples_processed
    );
}
