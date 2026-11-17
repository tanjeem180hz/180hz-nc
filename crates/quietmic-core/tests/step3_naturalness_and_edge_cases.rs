use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};
use std::f32::consts::PI;

#[test]
fn test_edge_cases_male_female_whisper_shout() {
    let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
    pipeline.set_mode(EngineMode::Standard);
    pipeline.set_strength(1.0);

    let mut out = Vec::with_capacity(490);

    // 1. Low-pitch male (80 Hz fundamental + harmonics)
    for _ in 0..10 {
        let male_frame: Vec<f32> = (0..480)
            .map(|i| {
                let t = i as f32 / 48000.0;
                0.25 * (2.0 * PI * 80.0 * t).sin()
                    + 0.15 * (2.0 * PI * 160.0 * t).sin()
                    + 0.10 * (2.0 * PI * 240.0 * t).sin()
            })
            .collect();
        pipeline.process_frame_internal(&male_frame, &mut out, 960).unwrap();
    }
    let male_energy: f32 = out.iter().map(|&s| s * s).sum();
    assert!(male_energy > 0.05, "Low-pitch male 80 Hz was excessively attenuated: {}", male_energy);

    // 2. High-pitch female (300 Hz fundamental + harmonics)
    for _ in 0..10 {
        let female_frame: Vec<f32> = (0..480)
            .map(|i| {
                let t = i as f32 / 48000.0;
                0.25 * (2.0 * PI * 300.0 * t).sin()
                    + 0.15 * (2.0 * PI * 600.0 * t).sin()
                    + 0.10 * (2.0 * PI * 900.0 * t).sin()
            })
            .collect();
        pipeline.process_frame_internal(&female_frame, &mut out, 960).unwrap();
    }
    let female_energy: f32 = out.iter().map(|&s| s * s).sum();
    assert!(female_energy > 0.05, "High-pitch female 300 Hz was excessively attenuated: {}", female_energy);

    // 3. Whispered speech (broadband unvoiced formant noise 1.5 kHz - 3.5 kHz at -35 dBFS)
    let mut lcg = 123456789u32;
    for _ in 0..15 {
        let whisper_frame: Vec<f32> = (0..480)
            .map(|i| {
                lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
                let white = (lcg as f32 / u32::MAX as f32) * 2.0 - 1.0;
                let t = i as f32 / 48000.0;
                white * 0.02 * (1.0 + (2.0 * PI * 2000.0 * t).sin())
            })
            .collect();
        pipeline.process_frame_internal(&whisper_frame, &mut out, 960).unwrap();
    }
    let whisper_energy: f32 = out.iter().map(|&s| s * s).sum();
    assert!(whisper_energy > 1e-5, "Whispered speech must not be gated to silence");

    // 4. Shouting / high amplitude (amplitude 2.5 = severe digital over-drive)
    let loud_frame: Vec<f32> = (0..480)
        .map(|i| (2.0 * PI * 400.0 * (i as f32 / 48000.0)).sin() * 2.5)
        .collect();
    pipeline.process_frame_internal(&loud_frame, &mut out, 960).unwrap();

    // Verify limiter smoothly prevents clipping above 1.05 without generating NaNs/Infs
    for &sample in &out {
        assert!(sample.is_finite(), "Shouting caused non-finite sample");
        assert!(sample.abs() <= 1.02, "Limiter failed to contain shout: {}", sample);
    }
}

#[test]
fn test_bluetooth_16khz_handsfree_resampling_pipeline() {
    // 16 kHz input and output (typical Bluetooth Hands-Free / Headset Profile)
    let mut pipeline = AudioPipeline::new(16000, 16000).expect("Pipeline 16kHz init failed");
    pipeline.set_mode(EngineMode::Standard);
    pipeline.set_strength(1.0);

    let frame_16k_len = 160; // 10 ms at 16 kHz
    let mut out_16k = Vec::with_capacity(frame_16k_len + 20);

    // Feed 20 frames of 16 kHz speech (400 Hz tone)
    for f in 0..20 {
        let frame: Vec<f32> = (0..frame_16k_len)
            .map(|i| {
                let t = (f * frame_16k_len + i) as f32 / 16000.0;
                (2.0 * PI * 400.0 * t).sin() * 0.3
            })
            .collect();

        assert!(pipeline.process_frame_internal(&frame, &mut out_16k, 320).is_ok());
    }

    // Steady state output must match 16 kHz frame size and contain audio
    assert_eq!(out_16k.len(), frame_16k_len, "Output length mismatch for 16 kHz frame");
    let energy: f32 = out_16k.iter().map(|&s| s * s).sum();
    assert!(energy > 0.01, "Bluetooth 16 kHz audio was lost during internal 48 kHz resampling");
}
