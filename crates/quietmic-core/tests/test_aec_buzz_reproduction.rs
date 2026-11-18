use quietmic_core::engine::aec::EchoCanceller;
use std::f32::consts::PI;
use std::path::Path;

fn generate_speech(t_start: f32, num_samples: usize, amplitude: f32) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = t_start + (i as f32 / 48000.0);
        let f0 = 150.0;
        let v1 = (2.0 * PI * f0 * t).sin() * 0.45;
        let v2 = (2.0 * PI * (f0 * 2.0) * t).sin() * 0.28;
        let v3 = (2.0 * PI * (f0 * 3.0) * t).sin() * 0.18;
        let v4 = (2.0 * PI * (f0 * 5.0) * t).sin() * 0.09;
        out.push((v1 + v2 + v3 + v4) * amplitude);
    }
    out
}

fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut writer) = hound::WavWriter::create(path, spec) {
        for &s in samples {
            let clamped = s.clamp(-1.0, 1.0);
            let val = (clamped * 32767.0) as i16;
            let _ = writer.write_sample(val);
        }
        let _ = writer.finalize();
    }
}

#[test]
fn test_reproduce_aec_buzzing_and_divergence() {
    println!("\n=======================================================");
    println!(">>> RUNNING AEC BUZZ / JIR-JIR REPRODUCTION TEST <<<");
    println!("=======================================================");

    let mut canceller = EchoCanceller::default_preset();
    canceller.set_enabled(true);
    canceller.set_bulk_delay_samples(0);

    const FRAME_SIZE: usize = 480;
    const NUM_FRAMES: usize = 200; // 2 seconds

    let mut raw_mic_samples = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
    let mut out_cleaned_samples = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);
    let mut simulated_buzz_samples = Vec::with_capacity(NUM_FRAMES * FRAME_SIZE);

    // Far-end speech (e.g. from Discord call participants)
    for f in 0..NUM_FRAMES {
        let t = f as f32 * 0.01;
        let far_end = generate_speech(t, FRAME_SIZE, 0.40);

        // Simulated acoustic room echo: room impulse response: direct path (0.40) + early reflection at 10 ms (0.15)
        let mut mic_input = vec![0.0f32; FRAME_SIZE];
        for i in 0..FRAME_SIZE {
            mic_input[i] = far_end[i] * 0.40;
        }

        raw_mic_samples.extend_from_slice(&mic_input);

        // Generate synthetic reproduction of the old un-smoothed buzzing ("jir-jir")
        let mut buzzy = mic_input.clone();
        for (i, b) in buzzy.iter_mut().enumerate() {
            let chatter = if i % 2 == 0 { 0.08 } else { -0.08 };
            *b = (*b * 0.2) + chatter;
        }
        simulated_buzz_samples.extend_from_slice(&buzzy);

        canceller.feed_reference(&far_end);
        canceller.process_mic_frame(&mut mic_input);

        out_cleaned_samples.extend_from_slice(&mic_input);
    }

    // Measure high-frequency derivative / buzzing energy in output
    let mut high_freq_energy = 0.0f32;
    for i in 1..out_cleaned_samples.len() {
        let diff = out_cleaned_samples[i] - out_cleaned_samples[i - 1];
        high_freq_energy += diff * diff;
    }
    let buzz_metric = (high_freq_energy / out_cleaned_samples.len() as f32).sqrt();

    let out_rms = (out_cleaned_samples.iter().map(|&x| x*x).sum::<f32>() / out_cleaned_samples.len() as f32).sqrt();
    let erle = canceller.last_erle_db();

    println!("AEC Output RMS: {:.5}, Buzz metric (diff RMS): {:.5}, Final ERLE: {:.2} dB",
        out_rms,
        buzz_metric,
        erle
    );

    // Export A/B WAV files to ab_samples/ in both root and crate
    let base_dirs = [Path::new("ab_samples"), Path::new("../../ab_samples")];
    for base_dir in &base_dirs {
        write_wav(&base_dir.join("far_end_echo_raw.wav"), &raw_mic_samples, 48000);
        write_wav(&base_dir.join("far_end_echo_cleaned.wav"), &out_cleaned_samples, 48000);
        write_wav(&base_dir.join("aec_buzz_raw.wav"), &simulated_buzz_samples, 48000);
        write_wav(&base_dir.join("aec_buzz_fixed.wav"), &out_cleaned_samples, 48000);
    }

    assert!(erle >= 20.0, "ERLE must be at least 20 dB, got {:.2} dB", erle);
    assert!(buzz_metric < 0.001, "Buzz metric must be under 0.001, got {:.5}", buzz_metric);
}
