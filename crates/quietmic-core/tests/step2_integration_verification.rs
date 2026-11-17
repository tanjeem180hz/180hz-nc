use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};

#[test]
fn test_pipeline_resampling_rates_consistency() {
    println!("\n=======================================================");
    println!(">>> STEP 2 CHECK 8: SAMPLE RATES & RESAMPLER BEHAVIOR <<<");
    println!("=======================================================");

    let test_rates = [(48000, 48000), (44100, 48000), (16000, 48000), (48000, 44100), (44100, 44100)];

    for (in_rate, out_rate) in test_rates {
        println!("\nTesting mic_rate={} Hz -> cable_rate={} Hz:", in_rate, out_rate);
        let mut pipeline = AudioPipeline::new(in_rate, out_rate).expect("Pipeline init failed");
        pipeline.set_mode(EngineMode::Light);
        pipeline.set_strength(1.0);

        let in_chunk_samples = ((in_rate as usize) / 100).max(1);
        let mut in_chunk = vec![0.0f32; in_chunk_samples];
        let mut out_chunk = Vec::new();

        // Feed 50 frames of noise
        let mut processed_frames = 0;
        let mut passthrough_detected = 0;

        for f in 0..50 {
            for (i, s) in in_chunk.iter_mut().enumerate() {
                let t = (f * in_chunk_samples + i) as f32 / in_rate as f32;
                *s = (2.0 * std::f32::consts::PI * 120.0 * t).sin() * 0.1;
            }

            let res = pipeline.process_frame_internal(&in_chunk, &mut out_chunk, 960);
            assert!(res.is_ok(), "Frame {} failed with error: {:?}", f, res);

            // Check if output was modified or directly passed through
            if out_chunk.len() == in_chunk.len() {
                let diff: f32 = out_chunk.iter().zip(&in_chunk).map(|(a, b)| (a - b).abs()).sum();
                if diff < 1e-4 {
                    passthrough_detected += 1;
                } else {
                    processed_frames += 1;
                }
            } else {
                processed_frames += 1;
            }
        }

        println!("  Frames fed: 50 | Processed: {} | Passthrough: {}", processed_frames, passthrough_detected);
        println!("  Output chunk length on last frame: {}", out_chunk.len());
    }
}
