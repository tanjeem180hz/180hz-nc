use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};

#[test]
fn test_wet_dry_mix_strength_0_50_100() {
    println!("\n=======================================================");
    println!(">>> STEP 2 CHECK 4: WET/DRY MIX STRENGTH 0, 50, 100 <<<");
    println!("=======================================================");

    let mut out_0 = Vec::new();
    let mut out_50 = Vec::new();
    let mut out_100 = Vec::new();

    let in_frame = vec![0.1f32; 480]; // Flat tone

    // Test strength 0.0 (100% Dry)
    let mut p0 = AudioPipeline::new(48000, 48000).unwrap();
    p0.set_mode(EngineMode::Light);
    p0.set_strength(0.0);
    // Warm up gain smoother
    for _ in 0..10 {
        p0.process_frame_internal(&in_frame, &mut out_0, 960).unwrap();
    }

    // Test strength 0.5 (50% Dry / 50% Wet)
    let mut p50 = AudioPipeline::new(48000, 48000).unwrap();
    p50.set_mode(EngineMode::Light);
    p50.set_strength(0.5);
    for _ in 0..10 {
        p50.process_frame_internal(&in_frame, &mut out_50, 960).unwrap();
    }

    // Test strength 1.0 (100% Cleaned)
    let mut p100 = AudioPipeline::new(48000, 48000).unwrap();
    p100.set_mode(EngineMode::Light);
    p100.set_strength(1.0);
    for _ in 0..10 {
        p100.process_frame_internal(&in_frame, &mut out_100, 960).unwrap();
    }

    let rms_0 = (out_0.iter().map(|&x| x * x).sum::<f32>() / 480.0).sqrt();
    let rms_50 = (out_50.iter().map(|&x| x * x).sum::<f32>() / 480.0).sqrt();
    let rms_100 = (out_100.iter().map(|&x| x * x).sum::<f32>() / 480.0).sqrt();

    println!("RMS at Strength 0%  (Dry):     {:.6}", rms_0);
    println!("RMS at Strength 50% (50/50):   {:.6}", rms_50);
    println!("RMS at Strength 100% (Cleaned): {:.6}", rms_100);

    assert!(rms_0 > rms_50, "Strength 0 RMS ({}) should be greater than 50% RMS ({})", rms_0, rms_50);
    assert!(rms_50 > rms_100, "Strength 50% RMS ({}) should be greater than 100% RMS ({})", rms_50, rms_100);
}
