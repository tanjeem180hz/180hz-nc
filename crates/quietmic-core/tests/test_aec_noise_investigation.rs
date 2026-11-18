use quietmic_core::engine::aec::EchoCanceller;

#[test]
fn test_aec_noise_floor_when_reference_is_silent() {
    let mut aec = EchoCanceller::default_preset();
    aec.set_enabled(true);

    // Simulated microphone input: constant fan noise (-35 dBFS RMS)
    let frame_size = 480;
    let mut mic_frame = vec![0.0f32; frame_size];
    let mut seed = 12345u32;
    for s in mic_frame.iter_mut() {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let n = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        *s = n * 0.018; // ~ -35 dBFS
    }

    let raw_rms: f32 = (mic_frame.iter().map(|&x| x * x).sum::<f32>() / frame_size as f32).sqrt();

    // Process 100 frames with silence fed as reference
    let silent_ref = vec![0.0f32; frame_size];
    let mut processed_frame = mic_frame.clone();

    for _ in 0..100 {
        aec.feed_reference(&silent_ref);
        processed_frame = mic_frame.clone();
        aec.process_mic_frame(&mut processed_frame);
    }

    let aec_rms: f32 = (processed_frame.iter().map(|&x| x * x).sum::<f32>() / frame_size as f32).sqrt();
    let delta_db = 20.0 * (aec_rms / raw_rms).log10();
    println!("AEC Noise Floor Test: raw_rms={:.6}, aec_rms={:.6}, delta_db={:.2} dB", raw_rms, aec_rms, delta_db);
}
use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};

#[test]
fn test_loud_speech_dropouts_and_clamping() {
    let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
    pipeline.set_mode(EngineMode::Standard);
    pipeline.set_strength(1.0);

    let frame_size = 480;
    let mut out = Vec::new();

    // Warm up pipeline with 20 frames of silence
    let silence = vec![0.0f32; frame_size];
    for _ in 0..20 {
        pipeline.process_frame_internal(&silence, &mut out, 0).unwrap();
    }

    // Now send loud speech: 440 Hz + 880 Hz harmonic vowel at -3 dBFS peak (amplitude 0.70)
    let mut num_dropouts = 0;
    let mut num_clamped_samples = 0;
    let mut input_peaks = Vec::new();
    let mut output_peaks = Vec::new();

    for frame_idx in 0..50 {
        let mut frame = vec![0.0f32; frame_size];
        for (i, s) in frame.iter_mut().enumerate() {
            let t = (frame_idx * frame_size + i) as f32 / 48000.0;
            // Harmonic speech-like vowel: fundamental 220Hz + harmonics at 440Hz, 880Hz, 1760Hz
            let val = 0.40 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
                    + 0.25 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                    + 0.15 * (2.0 * std::f32::consts::PI * 880.0 * t).sin()
                    + 0.10 * (2.0 * std::f32::consts::PI * 1760.0 * t).sin();
            *s = val; // Peak ~ 0.75 (-2.5 dBFS)
        }

        let in_peak = frame.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        input_peaks.push(in_peak);

        pipeline.process_frame_internal(&frame, &mut out, 0).unwrap();

        let out_peak = out.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        output_peaks.push(out_peak);

        // Check if output dropped by more than 6 dB relative to input
        if in_peak > 0.1 && (out_peak < in_peak * 0.5) {
            num_dropouts += 1;
        }

        // Check if any sample was hard-clamped around 0.10
        for &s in out.iter() {
            if (s.abs() - 0.10).abs() < 1e-4 {
                num_clamped_samples += 1;
            }
        }
    }

    let avg_in_peak = input_peaks.iter().sum::<f32>() / input_peaks.len() as f32;
    let avg_out_peak = output_peaks.iter().sum::<f32>() / output_peaks.len() as f32;

    println!(
        "Loud Speech Test: avg_in_peak={:.3}, avg_out_peak={:.3}, dropouts={}, clamped_samples={}",
        avg_in_peak, avg_out_peak, num_dropouts, num_clamped_samples
    );
    println!("Output peaks first 10 frames: {:?}", &output_peaks[..10]);
}
#[test]
fn test_loud_speech_light_mode_dropouts() {
    let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
    pipeline.set_mode(EngineMode::Light);
    pipeline.set_strength(1.0);

    let frame_size = 480;
    let mut out = Vec::new();

    let silence = vec![0.0f32; frame_size];
    for _ in 0..20 {
        pipeline.process_frame_internal(&silence, &mut out, 0).unwrap();
    }

    let mut num_dropouts = 0;
    let mut num_clamped_samples = 0;
    let mut output_peaks = Vec::new();

    for frame_idx in 0..50 {
        let mut frame = vec![0.0f32; frame_size];
        for (i, s) in frame.iter_mut().enumerate() {
            let t = (frame_idx * frame_size + i) as f32 / 48000.0;
            let val = 0.50 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
                    + 0.30 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                    + 0.15 * (2.0 * std::f32::consts::PI * 880.0 * t).sin();
            *s = val;
        }

        let in_peak = frame.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        pipeline.process_frame_internal(&frame, &mut out, 0).unwrap();

        let out_peak = out.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        output_peaks.push(out_peak);

        if in_peak > 0.1 && (out_peak < in_peak * 0.5) {
            num_dropouts += 1;
        }
        for &s in out.iter() {
            if (s.abs() - 0.10).abs() < 1e-4 {
                num_clamped_samples += 1;
            }
        }
    }

    println!(
        "Light Mode Loud Speech Test: dropouts={}, clamped_samples={}",
        num_dropouts, num_clamped_samples
    );
    println!("Light Mode Output peaks first 10 frames: {:?}", &output_peaks[..10]);
}
#[test]
fn test_aec_near_end_speech_weight_corruption() {
    let mut aec = EchoCanceller::default_preset();
    aec.set_enabled(true);

    let frame_size = 480;
    // Faint reference noise (~ -50 dBFS, typical DAC/soundcard loopback noise floor)
    let ref_noise: Vec<f32> = (0..frame_size).map(|i| (i as f32 * 0.3).sin() * 0.003).collect();

    // Loud near-end speech (-6 dBFS, amplitude 0.5)
    let loud_speech: Vec<f32> = (0..frame_size).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();

    // Process 50 frames of loud speech
    for _ in 0..50 {
        aec.feed_reference(&ref_noise);
        let mut mic = loud_speech.clone();
        aec.process_mic_frame(&mut mic);
    }

    // Now test what happens in silence right after loud speech
    let silence = vec![0.0f32; frame_size];
    aec.feed_reference(&ref_noise);
    let mut mic_silence = silence.clone();
    aec.process_mic_frame(&mut mic_silence);

    let out_noise_rms: f32 = (mic_silence.iter().map(|&x| x * x).sum::<f32>() / frame_size as f32).sqrt();
    let in_noise_rms: f32 = (silence.iter().map(|&x| x * x).sum::<f32>() / frame_size as f32).sqrt();

    println!("After loud speech, silence output RMS: in={:.8}, out={:.8}", in_noise_rms, out_noise_rms);
}
#[test]
fn test_aec_acceptance_noise_floor_delta_le_1db() {
    let mut aec = EchoCanceller::default_preset();
    aec.set_enabled(true);

    let frame_size = 480;
    // Ambient noise (fan + room hiss at -30 dBFS)
    let mut noise_signal = vec![0.0f32; frame_size * 20];
    let mut seed = 424242u32;
    for s in noise_signal.iter_mut() {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let n = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        *s = n * 0.0316; // -30 dBFS
    }

    let raw_rms: f32 = (noise_signal.iter().map(|&x| x * x).sum::<f32>() / noise_signal.len() as f32).sqrt();

    // Process all 20 frames through AEC with silence/low-level ambient reference
    let ref_dither: Vec<f32> = (0..frame_size).map(|i| (i as f32 * 0.1).sin() * 0.001).collect();
    let mut processed_signal = noise_signal.clone();

    for chunk in processed_signal.chunks_mut(frame_size) {
        aec.feed_reference(&ref_dither);
        aec.process_mic_frame(chunk);
    }

    let aec_rms: f32 = (processed_signal.iter().map(|&x| x * x).sum::<f32>() / processed_signal.len() as f32).sqrt();
    let delta_db = 20.0 * (aec_rms / raw_rms).log10();

    println!("AEC Acceptance Test: raw_rms={:.6}, aec_rms={:.6}, delta_db={:.2} dB", raw_rms, aec_rms, delta_db);
    assert!(
        delta_db <= 1.0,
        "AEC ON must never increase noise floor by > 1.0 dB! delta_db = {:.2} dB",
        delta_db
    );
}
#[test]
fn test_check_consecutive_clamped_samples() {
    let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
    pipeline.set_mode(EngineMode::Standard);
    pipeline.set_strength(1.0);

    let frame_size = 480;
    let mut out = Vec::new();

    let silence = vec![0.0f32; frame_size];
    for _ in 0..20 {
        pipeline.process_frame_internal(&silence, &mut out, 0).unwrap();
    }

    let mut flat_clipped_count = 0;

    for frame_idx in 0..50 {
        let mut frame = vec![0.0f32; frame_size];
        for (i, s) in frame.iter_mut().enumerate() {
            let t = (frame_idx * frame_size + i) as f32 / 48000.0;
            let val = 0.50 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
                    + 0.25 * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            *s = val;
        }

        pipeline.process_frame_internal(&frame, &mut out, 0).unwrap();

        for i in 1..out.len() {
            if (out[i] - out[i - 1]).abs() < 1e-7 && out[i].abs() > 0.05 {
                flat_clipped_count += 1;
            }
        }
    }

    println!("Consecutive flat clipped samples count: {}", flat_clipped_count);
    assert_eq!(flat_clipped_count, 0, "Speech waveform must NEVER have flat-topped clipped samples!");
}
