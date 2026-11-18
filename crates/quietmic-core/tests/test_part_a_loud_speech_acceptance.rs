use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};

#[test]
fn test_part_a_loud_speech_acceptance_across_levels() {
    let levels_dbfs = [-30.0f32, -20.0, -12.0, -6.0, -3.0];

    for &mode in &[EngineMode::Standard, EngineMode::Light] {
        for &dbfs in &levels_dbfs {
            let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
            pipeline.set_mode(mode);
            pipeline.set_strength(1.0);

            let peak_amp = 10.0f32.powf(dbfs / 20.0);
            let frame_size = 480;
            let mut out = Vec::new();

            // Warm up 10 frames of speech so recurrent engines are adapted
            for frame_idx in 0..10 {
                let frame: Vec<f32> = (0..frame_size).map(|i| {
                    let t = (frame_idx * frame_size + i) as f32 / 48000.0;
                    let val = 0.40 * (2.0 * std::f32::consts::PI * 180.0 * t).sin()
                            + 0.25 * (2.0 * std::f32::consts::PI * 360.0 * t).sin()
                            + 0.15 * (2.0 * std::f32::consts::PI * 540.0 * t).sin()
                            + 0.12 * (2.0 * std::f32::consts::PI * 720.0 * t).sin()
                            + 0.08 * (2.0 * std::f32::consts::PI * 900.0 * t).sin();
                    peak_amp * val
                }).collect();
                pipeline.process_frame_internal(&frame, &mut out, 0).unwrap();
            }

            // Now evaluate 30 steady-state frames
            let mut dropouts = 0;
            let mut clipped_samples = 0;
            let mut in_rms_total = 0.0f32;
            let mut out_rms_total = 0.0f32;

            for frame_idx in 10..40 {
                let frame: Vec<f32> = (0..frame_size).map(|i| {
                    let t = (frame_idx * frame_size + i) as f32 / 48000.0;
                    let val = 0.40 * (2.0 * std::f32::consts::PI * 180.0 * t).sin()
                            + 0.25 * (2.0 * std::f32::consts::PI * 360.0 * t).sin()
                            + 0.15 * (2.0 * std::f32::consts::PI * 540.0 * t).sin()
                            + 0.12 * (2.0 * std::f32::consts::PI * 720.0 * t).sin()
                            + 0.08 * (2.0 * std::f32::consts::PI * 900.0 * t).sin();
                    peak_amp * val
                }).collect();

                let in_rms = (frame.iter().map(|&x| x * x).sum::<f32>() / frame_size as f32).sqrt();
                pipeline.process_frame_internal(&frame, &mut out, 0).unwrap();
                let out_rms = (out.iter().map(|&x| x * x).sum::<f32>() / frame_size as f32).sqrt();

                in_rms_total += in_rms;
                out_rms_total += out_rms;

                // Dropout: energy drops > 6 dB (ratio < 0.50)
                if out_rms < in_rms * 0.50 {
                    dropouts += 1;
                }

                // Check for flat hard clipping
                for j in 1..out.len() {
                    if (out[j] - out[j - 1]).abs() < 1e-7 && out[j].abs() > 0.90 {
                        clipped_samples += 1;
                    }
                }
            }

            let avg_in_rms = in_rms_total / 30.0;
            let avg_out_rms = out_rms_total / 30.0;
            let delta_level_db = 20.0 * (avg_out_rms / avg_in_rms).log10();

            println!(
                "Mode {:?} at {:.1} dBFS: in_rms={:.4}, out_rms={:.4}, delta_db={:.2} dB, dropouts={}, clipped={}",
                mode, dbfs, avg_in_rms, avg_out_rms, delta_level_db, dropouts, clipped_samples
            );
        }
    }
}
use quietmic_core::engine::standard::StandardEngine;
use quietmic_core::engine::traits::DenoiseEngine;

#[test]
fn test_standard_engine_alone_at_minus_30dbfs() {
    let mut engine = StandardEngine::new();
    let peak_amp = 10.0f32.powf(-30.0 / 20.0);
    let frame_size = 480;
    let mut out = vec![0.0f32; frame_size];

    let mut in_rms_total = 0.0f32;
    let mut out_rms_total = 0.0f32;

    for frame_idx in 0..40 {
        let frame: Vec<f32> = (0..frame_size).map(|i| {
            let t = (frame_idx * frame_size + i) as f32 / 48000.0;
            let val = 0.40 * (2.0 * std::f32::consts::PI * 180.0 * t).sin()
                    + 0.25 * (2.0 * std::f32::consts::PI * 360.0 * t).sin()
                    + 0.15 * (2.0 * std::f32::consts::PI * 540.0 * t).sin()
                    + 0.12 * (2.0 * std::f32::consts::PI * 720.0 * t).sin()
                    + 0.08 * (2.0 * std::f32::consts::PI * 900.0 * t).sin();
            peak_amp * val
        }).collect();

        engine.process_frame(&frame, &mut out).unwrap();
        if frame_idx >= 10 {
            let in_rms = (frame.iter().map(|&x| x * x).sum::<f32>() / frame_size as f32).sqrt();
            let out_rms = (out.iter().map(|&x| x * x).sum::<f32>() / frame_size as f32).sqrt();
            in_rms_total += in_rms;
            out_rms_total += out_rms;
        }
    }

    let avg_in_rms = in_rms_total / 30.0;
    let avg_out_rms = out_rms_total / 30.0;
    let delta = 20.0 * (avg_out_rms / avg_in_rms).log10();
    println!("StandardEngine alone at -30 dBFS: in={:.5}, out={:.5}, delta={:.2} dB", avg_in_rms, avg_out_rms, delta);
}
