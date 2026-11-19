use quietmic_core::engine::aec::EchoCanceller;
use std::f32::consts::PI;

fn synthesize_speech_phonemes(num_samples: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / 48000.0;
        // Non-stationary pitch modulation (100 Hz to 280 Hz) and syllabic amplitude modulation (3 Hz syllables)
        let pitch = 140.0 + 70.0 * (2.0 * PI * 2.5 * t).sin() + 30.0 * (2.0 * PI * 7.1 * t).cos();
        let envelope = (2.0 * PI * 3.0 * t).sin().max(0.0).powf(1.8);
        
        let mut sample = 0.0f32;
        sample += (2.0 * PI * pitch * t).sin() * 0.40;
        sample += (2.0 * PI * (pitch * 2.0) * t).sin() * 0.25;
        sample += (2.0 * PI * (pitch * 3.5) * t).sin() * 0.15;
        sample += (2.0 * PI * 1800.0 * t).sin() * 0.08; // Formant
        sample += (2.0 * PI * 2600.0 * t).sin() * 0.05; // Formant
        
        out.push(sample * envelope * 0.5);
    }
    out
}

#[test]
fn test_aec_offline_real_speech_delays() {
    const FRAME_SIZE: usize = 480;
    const NUM_FRAMES: usize = 400; // 4 seconds of speech
    let speech = synthesize_speech_phonemes(NUM_FRAMES * FRAME_SIZE);

    let delays_ms = [20, 50, 80, 150];

    for &delay_ms in &delays_ms {
        let delay_samples = delay_ms * 48;
        println!("\n=== Real Non-Stationary Speech: Delay {} ms ({} samples) ===", delay_ms, delay_samples);

        // Case 1: Without delay alignment (bulk_delay = 0, as in live code currently)
        {
            let mut canceller = EchoCanceller::default_preset();
            canceller.set_enabled(true);
            canceller.set_bulk_delay_samples(0); // 0 delay alignment

            let mut raw_echo = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
            let mut clean_out = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);

            for f in 0..NUM_FRAMES {
                let start = f * FRAME_SIZE;
                let ref_frame = &speech[start..start + FRAME_SIZE];
                canceller.feed_reference(ref_frame);

                let mut mic_frame = vec![0.0f32; FRAME_SIZE];
                for i in 0..FRAME_SIZE {
                    let g = start + i;
                    if g >= delay_samples {
                        mic_frame[i] = speech[g - delay_samples] * 0.40;
                    }
                }
                raw_echo.extend_from_slice(&mic_frame);
                canceller.process_mic_frame(&mut mic_frame);
                clean_out.extend_from_slice(&mic_frame);
            }

            let eval_start = (NUM_FRAMES / 2) * FRAME_SIZE;
            let raw_rms = (raw_echo[eval_start..].iter().map(|&x| x * x).sum::<f32>() / (raw_echo.len() - eval_start) as f32).sqrt();
            let clean_rms = (clean_out[eval_start..].iter().map(|&x| x * x).sum::<f32>() / (clean_out.len() - eval_start) as f32).sqrt();
            let erle_db = 20.0 * (raw_rms / (clean_rms + 1e-6)).log10();
            println!("  [bulk_delay = 0]: Raw RMS: {:.4}, Clean RMS: {:.4}, ERLE: {:.2} dB", raw_rms, clean_rms, erle_db);
        }

        // Case 2: With exact delay alignment (bulk_delay = delay_samples)
        {
            let mut canceller = EchoCanceller::default_preset();
            canceller.set_enabled(true);
            canceller.set_bulk_delay_samples(delay_samples); // Aligned!

            let mut raw_echo = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
            let mut clean_out = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);

            for f in 0..NUM_FRAMES {
                let start = f * FRAME_SIZE;
                let ref_frame = &speech[start..start + FRAME_SIZE];
                canceller.feed_reference(ref_frame);

                let mut mic_frame = vec![0.0f32; FRAME_SIZE];
                for i in 0..FRAME_SIZE {
                    let g = start + i;
                    if g >= delay_samples {
                        mic_frame[i] = speech[g - delay_samples] * 0.40;
                    }
                }
                raw_echo.extend_from_slice(&mic_frame);
                canceller.process_mic_frame(&mut mic_frame);
                clean_out.extend_from_slice(&mic_frame);
            }

            let eval_start = (NUM_FRAMES / 2) * FRAME_SIZE;
            let raw_rms = (raw_echo[eval_start..].iter().map(|&x| x * x).sum::<f32>() / (raw_echo.len() - eval_start) as f32).sqrt();
            let clean_rms = (clean_out[eval_start..].iter().map(|&x| x * x).sum::<f32>() / (clean_out.len() - eval_start) as f32).sqrt();
            let erle_db = 20.0 * (raw_rms / (clean_rms + 1e-6)).log10();
            println!("  [bulk_delay = {}]: Raw RMS: {:.4}, Clean RMS: {:.5}, ERLE: {:.2} dB", delay_samples, raw_rms, clean_rms, erle_db);
        }
    }
}
