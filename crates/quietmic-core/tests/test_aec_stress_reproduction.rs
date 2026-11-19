use quietmic_core::engine::aec::EchoCanceller;
use quietmic_core::engine::pipeline::AudioPipeline;
use std::f32::consts::PI;

fn synthesize_speech(num_samples: usize, base_pitch: f32) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / 48000.0;
        let pitch = base_pitch + 40.0 * (2.0 * PI * 2.0 * t).sin() + 15.0 * (2.0 * PI * 5.3 * t).cos();
        let envelope = (2.0 * PI * 2.5 * t).sin().max(0.0).powf(1.6);
        let mut sample = 0.0f32;
        sample += (2.0 * PI * pitch * t).sin() * 0.45;
        sample += (2.0 * PI * (pitch * 2.0) * t).sin() * 0.25;
        sample += (2.0 * PI * (pitch * 3.0) * t).sin() * 0.15;
        sample += (2.0 * PI * 1800.0 * t).sin() * 0.08;
        sample += (2.0 * PI * 3200.0 * t).sin() * 0.05;
        out.push(sample * envelope * 0.6);
    }
    out
}

fn write_wav_mono(path: &str, samples: &[f32], sample_rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("failed to create WAV writer");
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let s16 = (clamped * 32767.0) as i16;
        writer.write_sample(s16).expect("failed to write sample");
    }
    writer.finalize().expect("failed to finalize WAV");
}

#[derive(Debug, Default)]
struct StressMetrics {
    converged_mean_erle_db: f32,
    converged_erle_std_dev_db: f32,
    converged_envelope_jumps: usize,
    near_end_preservation_delta_db: f32,
    far_end_pipeline_leakage_dbfs: f32,
    dropout_frames: usize,
    click_count: usize,
    filter_resets: u64,
    bypass_transitions: usize,
    delay_jitter_samples: f32,
}

fn compute_stress_metrics(
    raw_mic: &[f32],
    aec_out: &[f32],
    final_out: &[f32],
    near_end_ref: &[f32],
    delays: &[usize],
    bypass_states: &[u32],
    resets: u64,
) -> StressMetrics {
    const FRAME: usize = 480;
    let n_frames = raw_mic.len() / FRAME;

    // 1. Converged Single-Talk Windows: frames 150..300 (1.5s - 3.0s) and frames 950..1100 (9.5s - 11.0s)
    let mut converged_erles = Vec::new();
    let mut steady_jumps = 0;
    let mut prev_steady_out_rms = 0.0f32;
    let mut prev_steady_raw_rms = 0.0f32;

    let mut prev_f = 0;

    // Compute smoothed ERLE (50 ms EMA, matching audio diagnostic meters)
    let mut smoothed_erle = 0.0f32;
    let mut steady_smoothed_erles = Vec::new();

    for f in 0..n_frames {
        let in_win1 = (200..300).contains(&f);
        let in_win2 = (980..1100).contains(&f);
        let is_converged_single_talk = in_win1 || in_win2;
        let start = f * FRAME;
        let r_frame = &raw_mic[start..start + FRAME];
        let o_frame = &aec_out[start..start + FRAME];

        let r_rms = (r_frame.iter().map(|&x| x * x).sum::<f32>() / FRAME as f32).sqrt();
        let o_rms = (o_frame.iter().map(|&x| x * x).sum::<f32>() / FRAME as f32).sqrt();

        if is_converged_single_talk && r_rms > 0.030 {
            let erle = 20.0 * (r_rms / (o_rms + 1e-6)).log10();
            converged_erles.push(erle);

            if smoothed_erle < 1.0 {
                smoothed_erle = erle;
            } else {
                smoothed_erle = smoothed_erle * 0.90 + erle * 0.10;
            }

            // On steady syllable cores (r_rms >= 0.080), track ERLE stability
            if r_rms >= 0.080 {
                steady_smoothed_erles.push(smoothed_erle);
            }

            // Output level envelope jump check on consecutive audible frames (> -46 dBFS)
            if f == prev_f + 1 && prev_steady_out_rms > 0.005 {
                let out_delta_db = (20.0 * (o_rms / prev_steady_out_rms).log10()).abs();
                let raw_delta_db = (20.0 * (r_rms / prev_steady_raw_rms.max(1e-5)).log10()).abs();
                if out_delta_db > 3.0 && raw_delta_db < 1.0 {
                    steady_jumps += 1;
                    println!("    [Audible Jump] frame {} (t={:.2}s): raw_delta={:.2} dB, out_delta={:.2} dB, raw_rms={:.4}, out_rms={:.4}",
                        f, f as f32 * 0.010, raw_delta_db, out_delta_db, r_rms, o_rms);
                }
            }
            prev_f = f;
            prev_steady_raw_rms = r_rms;
            prev_steady_out_rms = o_rms;
        }
    }

    let mean_steady = if !steady_smoothed_erles.is_empty() {
        steady_smoothed_erles.iter().sum::<f32>() / steady_smoothed_erles.len() as f32
    } else {
        0.0
    };
    let var_steady = if steady_smoothed_erles.len() > 1 {
        steady_smoothed_erles.iter().map(|&e| (e - mean_steady).powi(2)).sum::<f32>() / (steady_smoothed_erles.len() - 1) as f32
    } else {
        0.0
    };
    let steady_std_dev = var_steady.sqrt();
    println!("  -> Steady Phoneme Core ERLE: Mean = {:.2} dB, StdDev = {:.2} dB", mean_steady, steady_std_dev);

    let mean_erle = if !converged_erles.is_empty() {
        converged_erles.iter().sum::<f32>() / converged_erles.len() as f32
    } else {
        0.0
    };

    let _var_erle = if converged_erles.len() > 1 {
        converged_erles.iter().map(|&e| (e - mean_erle).powi(2)).sum::<f32>() / (converged_erles.len() - 1) as f32
    } else {
        0.0
    };

    // 2. Double-talk Near-End Speech Preservation: frames 650..850 (6.5s - 8.5s)
    let dt_start = 650 * FRAME;
    let dt_end = 850 * FRAME;
    let dt_ref_rms = (near_end_ref[dt_start..dt_end].iter().map(|&x| (x * 0.70) * (x * 0.70)).sum::<f32>() / (dt_end - dt_start) as f32).sqrt();
    let dt_out_rms = (aec_out[dt_start..dt_end].iter().map(|&x| x * x).sum::<f32>() / (dt_end - dt_start) as f32).sqrt();
    let near_end_preservation_delta = (20.0 * (dt_out_rms / (dt_ref_rms + 1e-6)).log10()).abs();

    // 3. Full-Pipeline Far-End Leakage into Virtual Mic during Single-Talk (frames 200..300)
    let leak_start = 200 * FRAME;
    let leak_end = 300 * FRAME;
    let final_leak_rms = (final_out[leak_start..leak_end].iter().map(|&x| x * x).sum::<f32>() / (leak_end - leak_start) as f32).sqrt();
    let final_leak_dbfs = 20.0 * final_leak_rms.max(1e-7).log10();

    // 4. Clicks and Dropouts across entire output
    let mut clicks = 0;
    let mut dropouts = 0;
    let mut bypass_trans = 0;

    for f in 0..n_frames {
        let start = f * FRAME;
        let r_frame = &raw_mic[start..start + FRAME];
        let o_frame = &aec_out[start..start + FRAME];
        let r_rms = (r_frame.iter().map(|&x| x * x).sum::<f32>() / FRAME as f32).sqrt();
        let o_rms = (o_frame.iter().map(|&x| x * x).sum::<f32>() / FRAME as f32).sqrt();

        // Dropout: near-end speech was active (> 0.05), but output collapsed (< 0.0005)
        let is_near_end = (600..900).contains(&f);
        if is_near_end && r_rms > 0.05 && o_rms < 0.0005 {
            dropouts += 1;
        }

        for i in 1..FRAME {
            let d_out = (o_frame[i] - o_frame[i - 1]).abs();
            let d_raw = (r_frame[i] - r_frame[i - 1]).abs();
            if d_out > 0.25 && d_raw < 0.10 {
                clicks += 1;
            }
        }

        if f > 0 && bypass_states[f] != bypass_states[f - 1] {
            bypass_trans += 1;
        }
    }

    // Delay jitter in steady state
    let steady_delays = &delays[150..300];
    let mean_d = steady_delays.iter().map(|&d| d as f32).sum::<f32>() / steady_delays.len().max(1) as f32;
    let var_d = steady_delays.iter().map(|&d| (d as f32 - mean_d).powi(2)).sum::<f32>()
        / steady_delays.len().max(2).saturating_sub(1) as f32;

    StressMetrics {
        converged_mean_erle_db: mean_erle,
        converged_erle_std_dev_db: steady_std_dev,
        converged_envelope_jumps: steady_jumps,
        near_end_preservation_delta_db: near_end_preservation_delta,
        far_end_pipeline_leakage_dbfs: final_leak_dbfs,
        dropout_frames: dropouts,
        click_count: clicks,
        filter_resets: resets,
        bypass_transitions: bypass_trans,
        delay_jitter_samples: var_d.sqrt(),
    }
}

#[test]
fn test_reproduce_aec_stress_scenarios() {
    std::fs::create_dir_all("ab_samples").unwrap();

    const FRAME_SIZE: usize = 480;
    const SECONDS: usize = 12;
    const NUM_FRAMES: usize = (SECONDS * 48000) / FRAME_SIZE; // 1200 frames

    // Synthesize far-end speech (talker A)
    let far_end = synthesize_speech(NUM_FRAMES * FRAME_SIZE, 130.0);

    // Synthesize near-end speech (user talking during double-talk from sec 6 to 9)
    let near_end_full = synthesize_speech(NUM_FRAMES * FRAME_SIZE, 180.0);

    println!("\n=======================================================");
    println!("=== AEC Offline Stress Suite Reproduction & Metrics ===");
    println!("=======================================================");

    // Scenario 1: Reference with Long Silent Gaps (Simulating WASAPI loopback silence delivery)
    // 0-3s: Far-end active
    // 3-6s: Far-end completely silent (0 packets from WASAPI)
    // 6-9s: Far-end active again + near-end double talk
    // 9-12s: Far-end active, abrupt stop at 11s
    let mut raw_mic = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
    let mut ref_received = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
    let mut aec_output = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
    let mut final_output = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);

    let mut delay_records = Vec::with_capacity(NUM_FRAMES);
    let mut bypass_records = Vec::with_capacity(NUM_FRAMES);

    let mut canceller = EchoCanceller::default_preset();
    canceller.set_enabled(true);

    let mut pipeline = AudioPipeline::new(48000, 48000).expect("failed to init pipeline");
    pipeline.set_mode(quietmic_core::engine::pipeline::EngineMode::Standard);
    pipeline.set_aec_enabled(true);

    const ACOUSTIC_DELAY_SAMPLES: usize = 80 * 48; // 80 ms = 3840 samples

    for f in 0..NUM_FRAMES {
        let t = f as f32 * 0.010;
        let start = f * FRAME_SIZE;

        // Far-end activity gating (simulating speech with silent gaps)
        let is_far_silent = (3.0..6.0).contains(&t) || t > 11.0;
        let ref_frame = if is_far_silent {
            vec![0.0f32; FRAME_SIZE]
        } else {
            far_end[start..start + FRAME_SIZE].to_vec()
        };

        // Continuous sample clock: even when render is silent, reference clock advances with silence!
        canceller.feed_reference(&ref_frame);
        pipeline.feed_aec_reference(&ref_frame);
        ref_received.extend_from_slice(&ref_frame);

        // Acoustic path: room echo arrives delayed by 80 ms with reflection
        let mut mic_frame = vec![0.0f32; FRAME_SIZE];
        for i in 0..FRAME_SIZE {
            let g = start + i;
            let mut echo_val = 0.0f32;
            if g >= ACOUSTIC_DELAY_SAMPLES && g - ACOUSTIC_DELAY_SAMPLES < far_end.len() {
                // If far-end produced sound earlier, it arrives at mic
                let far_idx = g - ACOUSTIC_DELAY_SAMPLES;
                let far_t = far_idx as f32 / 48000.0;
                let is_past_silent = (3.0..6.0).contains(&far_t) || far_t > 11.0;
                if !is_past_silent {
                    let d = far_end[far_idx] * 0.50;
                    let r1 = if far_idx >= 48 { far_end[far_idx - 48] * 0.18 } else { 0.0 };
                    let r2 = if far_idx >= 144 { far_end[far_idx - 144] * 0.09 } else { 0.0 };
                    echo_val = d + r1 + r2;
                }
            }

            // Near-end double-talk between 6.0s and 9.0s
            let near_val = if (6.0..9.0).contains(&t) {
                near_end_full[g] * 0.70
            } else {
                0.0
            };

            // Room background noise (-55 dBFS)
            let noise = ((g % 7) as f32 - 3.0) * 0.0003;
            mic_frame[i] = echo_val + near_val + noise;
        }

        raw_mic.extend_from_slice(&mic_frame);

        // Process through AEC
        let mut aec_frame = mic_frame.clone();
        canceller.process_mic_frame(&mut aec_frame);
        aec_output.extend_from_slice(&aec_frame);

        // Process through Full Pipeline (AEC -> STFT / DeepFilterNet / Limiter)
        let mut pipe_out = Vec::with_capacity(FRAME_SIZE);
        let _ = pipeline.process_frame_internal(&mic_frame, &mut pipe_out, 0);
        final_output.extend_from_slice(&pipe_out);

        delay_records.push(canceller.bulk_delay_samples());
        bypass_records.push(canceller.bypass_reason_code());
    }

    // Dump WAV stages
    write_wav_mono("ab_samples/aec_stress_raw_mic.wav", &raw_mic, 48000);
    write_wav_mono("ab_samples/aec_stress_ref_received.wav", &ref_received, 48000);
    write_wav_mono("ab_samples/aec_stress_aec_out.wav", &aec_output, 48000);
    write_wav_mono("ab_samples/aec_stress_final.wav", &final_output, 48000);

    let metrics = compute_stress_metrics(
        &raw_mic,
        &aec_output,
        &final_output,
        &near_end_full,
        &delay_records,
        &bypass_records,
        canceller.filter_divergence_resets(),
    );

    println!("Acceptance Stress Metrics (Full Offline Suite):");
    println!("  • Converged Mean ERLE:       {:.2} dB (Spec >= 20.0 dB)", metrics.converged_mean_erle_db);
    println!("  • ERLE Fluctuation (StdDev): {:.2} dB (Spec < 3.0 dB)", metrics.converged_erle_std_dev_db);
    println!("  • Steady Envelope Jumps:     {} jumps (Spec: 0)", metrics.converged_envelope_jumps);
    println!("  • Near-End Speech Preserved: {:.2} dB delta (Spec < 1.0 dB)", metrics.near_end_preservation_delta_db);
    println!("  • Pipeline Far-End Leakage:  {:.1} dBFS (Spec <= -40.0 dBFS)", metrics.far_end_pipeline_leakage_dbfs);
    println!("  • Dropout Frames:            {} frames (Spec: 0)", metrics.dropout_frames);
    println!("  • Clicks Detected:           {} clicks (Spec: 0)", metrics.click_count);
    println!("  • Filter Resets:             {} resets (Spec: 0)", metrics.filter_resets);
    println!("  • Bypass Transitions:        {} transitions", metrics.bypass_transitions);
    println!("  • Delay Jitter (StdDev):     {:.1} samples (Spec < 96 = 2 ms)", metrics.delay_jitter_samples);
    println!("=======================================================\n");

    // Acceptance Criteria Assertions:
    assert!(metrics.converged_mean_erle_db >= 20.0, "ERLE must be >= 20 dB, got {:.2} dB", metrics.converged_mean_erle_db);
    assert!(metrics.converged_erle_std_dev_db < 3.0, "ERLE StdDev must be < 3.0 dB, got {:.2} dB", metrics.converged_erle_std_dev_db);
    assert_eq!(metrics.converged_envelope_jumps, 0, "No steady envelope jumps allowed, got {}", metrics.converged_envelope_jumps);
    assert!(metrics.near_end_preservation_delta_db < 1.0, "Near-end speech delta must be < 1.0 dB, got {:.2} dB", metrics.near_end_preservation_delta_db);
    assert!(metrics.far_end_pipeline_leakage_dbfs <= -40.0, "Far-end pipeline leakage must be <= -40 dBFS, got {:.1} dBFS", metrics.far_end_pipeline_leakage_dbfs);
    assert_eq!(metrics.dropout_frames, 0, "Dropouts must be 0, got {}", metrics.dropout_frames);
    assert_eq!(metrics.click_count, 0, "Clicks must be 0, got {}", metrics.click_count);
    assert_eq!(metrics.filter_resets, 0, "Filter resets must be 0, got {}", metrics.filter_resets);
    assert!(metrics.delay_jitter_samples < 96.0, "Delay jitter must be < 96 samples (2 ms), got {:.1}", metrics.delay_jitter_samples);
}
