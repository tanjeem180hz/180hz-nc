use quietmic_core::engine::aec::EchoCanceller;
use std::f32::consts::PI;

fn synthesize_speech_phonemes(num_samples: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / 48000.0;
        let pitch = 140.0 + 70.0 * (2.0 * PI * 2.5 * t).sin() + 30.0 * (2.0 * PI * 7.1 * t).cos();
        let envelope = (2.0 * PI * 3.0 * t).sin().max(0.0).powf(1.8);
        
        let mut sample = 0.0f32;
        sample += (2.0 * PI * pitch * t).sin() * 0.40;
        sample += (2.0 * PI * (pitch * 2.0) * t).sin() * 0.25;
        sample += (2.0 * PI * (pitch * 3.5) * t).sin() * 0.15;
        sample += (2.0 * PI * 1800.0 * t).sin() * 0.08;
        sample += (2.0 * PI * 2600.0 * t).sin() * 0.05;
        
        out.push(sample * envelope * 0.5);
    }
    out
}

fn write_wav_file(path: &str, samples: &[f32], sample_rate: u32) {
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

#[test]
fn test_aec_delays_20_80_150_300ms_erle_ge_20db() {
    const FRAME_SIZE: usize = 480;
    const NUM_FRAMES: usize = 400; // 4 seconds of speech
    let speech = synthesize_speech_phonemes(NUM_FRAMES * FRAME_SIZE);

    let test_delays_ms = [20, 80, 150, 300];

    for &delay_ms in &test_delays_ms {
        let delay_samples = delay_ms * 48;
        println!("\n=== Testing Real-World AEC Acceptance: Delay {} ms ({} samples) ===", delay_ms, delay_samples);

        let mut canceller = EchoCanceller::default_preset();
        canceller.set_enabled(true);

        let mut raw_echo = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
        let mut clean_out = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);

        for f in 0..NUM_FRAMES {
            let start = f * FRAME_SIZE;
            let ref_frame = &speech[start..start + FRAME_SIZE];
            canceller.feed_reference(ref_frame);

            let mut mic_frame = vec![0.0f32; FRAME_SIZE];
            for i in 0..FRAME_SIZE {
                let g = start + i;
                if g >= delay_samples && g - delay_samples < speech.len() {
                    let direct = speech[g - delay_samples] * 0.40;
                    let ref1 = if g >= delay_samples + 48 { speech[g - delay_samples - 48] * 0.15 } else { 0.0 };
                    let ref2 = if g >= delay_samples + 144 { speech[g - delay_samples - 144] * 0.08 } else { 0.0 };
                    mic_frame[i] = direct + ref1 + ref2;
                }
            }
            raw_echo.extend_from_slice(&mic_frame);
            canceller.process_mic_frame(&mut mic_frame);
            clean_out.extend_from_slice(&mic_frame);
        }

        // Measure ERLE on converged interval (last 2 seconds)
        let eval_start = (NUM_FRAMES / 2) * FRAME_SIZE;
        let raw_rms = (raw_echo[eval_start..].iter().map(|&x| x * x).sum::<f32>() / (raw_echo.len() - eval_start) as f32).sqrt();
        let clean_rms = (clean_out[eval_start..].iter().map(|&x| x * x).sum::<f32>() / (clean_out.len() - eval_start) as f32).sqrt();
        let erle_db = 20.0 * (raw_rms / (clean_rms + 1e-6)).log10();

        println!(
            "  Delay {} ms: Locked Delay = {} ms ({} samples), Raw RMS = {:.4}, Clean RMS = {:.5}, ERLE = {:.2} dB",
            delay_ms, canceller.bulk_delay_ms(), canceller.bulk_delay_samples(), raw_rms, clean_rms, erle_db
        );

        assert!(
            canceller.is_delay_locked(),
            "AEC delay tracker failed to lock delay for {} ms",
            delay_ms
        );
        let delay_error = (canceller.bulk_delay_samples() as i64 - delay_samples as i64).abs();
        assert!(
            delay_error <= 48,
            "Delay tracking error {} samples too high for {} ms",
            delay_error, delay_ms
        );
        assert!(
            erle_db >= 20.0,
            "ERLE {:.2} dB did not meet >= 20.0 dB requirement for {} ms",
            erle_db, delay_ms
        );

        // Save representative sample for delay 80 ms
        if delay_ms == 80 {
            write_wav_file("ab_samples/aec_acceptance_raw.wav", &raw_echo, 48000);
            write_wav_file("ab_samples/aec_acceptance_cleaned.wav", &clean_out, 48000);
        }
    }
}

#[test]
fn test_aec_noise_floor_delta_le_1db() {
    // When far-end is silent or soundcard dither, AEC must NOT increase noise floor by > 1.0 dB
    let num_samples = 48000 * 2;
    let mut rng = 123456789u32;
    let mut dither_noise = Vec::with_capacity(num_samples);
    for _ in 0..num_samples {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let s = (rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
        dither_noise.push(s * 0.001); // -60 dBFS quiet room noise
    }

    let mut canceller = EchoCanceller::default_preset();
    canceller.set_enabled(true);

    let frame_size = 480;
    let mut processed = dither_noise.clone();
    let silent_ref = vec![0.0f32; frame_size];

    for chunk in processed.chunks_mut(frame_size) {
        canceller.feed_reference(&silent_ref);
        canceller.process_mic_frame(chunk);
    }

    let raw_rms = (dither_noise.iter().map(|&x| x * x).sum::<f32>() / dither_noise.len() as f32).sqrt();
    let proc_rms = (processed.iter().map(|&x| x * x).sum::<f32>() / processed.len() as f32).sqrt();
    let delta_db = (20.0 * (proc_rms / (raw_rms + 1e-9)).log10()).abs();

    println!("AEC Noise Floor Test: Raw RMS = {:.6}, Proc RMS = {:.6}, Delta = {:.4} dB", raw_rms, proc_rms, delta_db);
    assert!(delta_db <= 1.0, "AEC altered noise floor by {:.2} dB (target <= 1.0 dB)", delta_db);
}
