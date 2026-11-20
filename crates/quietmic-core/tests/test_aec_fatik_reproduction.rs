use quietmic_core::engine::aec::EchoCanceller;
use std::f32::consts::PI;

fn synthesize_speech_with_consonants(num_samples: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / 48000.0;
        // Syllable rate: 3 syllables per second
        let syllable_phase = (2.0 * PI * 3.0 * t).sin();
        let is_vowel = syllable_phase > 0.1;
        let is_consonant = syllable_phase > -0.6 && syllable_phase <= 0.1;

        let sample = if is_vowel {
            // Vowel formant
            let f0 = 130.0;
            0.50 * (2.0 * PI * f0 * t).sin()
                + 0.30 * (2.0 * PI * (f0 * 2.0) * t).sin()
                + 0.20 * (2.0 * PI * 750.0 * t).sin()
                + 0.15 * (2.0 * PI * 1200.0 * t).sin()
        } else if is_consonant {
            // High-frequency fricative / stop consonant ("s", "t", "sh") - low energy, high frequency
            let noise = (((i * 1103515245 + 12345) % 32768) as f32 / 16384.0) - 1.0;
            noise * 0.08 + 0.05 * (2.0 * PI * 3500.0 * t).sin()
        } else {
            // Inter-syllable pause
            0.0
        };
        out.push(sample * 0.50);
    }
    out
}

fn synthesize_far_end_music(num_samples: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / 48000.0;
        let f1 = 220.0;
        let s = (2.0 * PI * f1 * t).sin() * 0.35 + (2.0 * PI * (f1 * 1.5) * t).sin() * 0.20;
        out.push(s);
    }
    out
}

#[test]
fn test_reproduce_aec_fatik_distortion() {
    println!("\n=======================================================");
    println!(">>> RUNNING AEC 'FATIK' CHOPPY SPEECH VERIFICATION <<<");
    println!("=======================================================");

    let mut canceller = EchoCanceller::default_preset();
    canceller.set_enabled(true);
    canceller.set_bulk_delay_samples(480); // 10 ms hardware delay

    const FRAME_SIZE: usize = 480;
    const NUM_FRAMES: usize = 300; // 3 seconds of audio

    let far_end = synthesize_far_end_music(NUM_FRAMES * FRAME_SIZE);
    let near_end_full = synthesize_speech_with_consonants(NUM_FRAMES * FRAME_SIZE);

    let mut raw_mic = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
    let mut aec_out = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);

    let mut dtd_switch_count = 0;
    let mut prev_dtd = false;

    for f in 0..NUM_FRAMES {
        let start = f * FRAME_SIZE;
        let ref_frame = &far_end[start..start + FRAME_SIZE];
        canceller.feed_reference(ref_frame);

        // Acoustic path: direct echo (0.30) + reflection (0.12 at 10 ms delay = 480 samples)
        let mut mic_frame = vec![0.0f32; FRAME_SIZE];
        for i in 0..FRAME_SIZE {
            let g = start + i;
            let echo = if g >= 480 { far_end[g - 480] * 0.30 } else { 0.0 };
            // Speech starts at frame 100 (1.0s), allowing initial 1 second for single-talk echo adaptation
            let speech = if f >= 100 { near_end_full[g] } else { 0.0 };
            mic_frame[i] = echo + speech;
        }

        raw_mic.extend_from_slice(&mic_frame);

        // Process frame
        canceller.process_mic_frame(&mut mic_frame);

        let dtd_now = canceller.is_double_talk();
        if dtd_now != prev_dtd {
            dtd_switch_count += 1;
            prev_dtd = dtd_now;
        }

        aec_out.extend_from_slice(&mic_frame);
    }

    // 1. Single-talk convergence ERLE (frames 60..99)
    let st_raw_rms = (raw_mic[60 * FRAME_SIZE..100 * FRAME_SIZE].iter().map(|&x| x * x).sum::<f32>() / (40 * FRAME_SIZE) as f32).sqrt();
    let st_out_rms = (aec_out[60 * FRAME_SIZE..100 * FRAME_SIZE].iter().map(|&x| x * x).sum::<f32>() / (40 * FRAME_SIZE) as f32).sqrt();
    let single_talk_erle = 20.0 * (st_raw_rms / (st_out_rms + 1e-6)).log10();

    // 2. Double-talk speech preservation (frames 120..280)
    let dt_speech_rms = (near_end_full[120 * FRAME_SIZE..280 * FRAME_SIZE].iter().map(|&x| x * x).sum::<f32>() / (160 * FRAME_SIZE) as f32).sqrt();
    let dt_out_rms = (aec_out[120 * FRAME_SIZE..280 * FRAME_SIZE].iter().map(|&x| x * x).sum::<f32>() / (160 * FRAME_SIZE) as f32).sqrt();
    let speech_delta_db = (20.0 * (dt_out_rms / (dt_speech_rms + 1e-6)).log10()).abs();

    println!("Single-Talk Converged ERLE: {:.2} dB (Spec >= 20.0 dB)", single_talk_erle);
    println!("Double-Talk Speech Level Delta: {:.2} dB (Spec < 1.5 dB)", speech_delta_db);
    println!("DTD state switches over 3.0s: {} switches", dtd_switch_count);

    // Export test WAVs to ab_samples/
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    std::fs::create_dir_all("ab_samples").ok();
    if let Ok(mut w) = hound::WavWriter::create("ab_samples/fatik_raw_mic.wav", spec) {
        for &s in &raw_mic {
            let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
        let _ = w.finalize();
    }
    if let Ok(mut w) = hound::WavWriter::create("ab_samples/fatik_near_speech.wav", spec) {
        for &s in &near_end_full {
            let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
        let _ = w.finalize();
    }
    if let Ok(mut w) = hound::WavWriter::create("ab_samples/fatik_aec_out.wav", spec) {
        for &s in &aec_out {
            let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
        let _ = w.finalize();
    }

    assert!(single_talk_erle >= 20.0, "Single-talk ERLE must be >= 20.0 dB, got {:.2} dB", single_talk_erle);
    assert!(speech_delta_db < 1.5, "Speech level distortion must be < 1.5 dB, got {:.2} dB", speech_delta_db);
}
