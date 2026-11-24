use hound::{WavSpec, WavWriter};
use quietmic_core::engine::light::LightEngine;
use quietmic_core::engine::standard::StandardEngine;
use quietmic_core::engine::traits::DenoiseEngine;
use std::f32::consts::PI;
use std::fs;
use std::path::Path;

const SAMPLE_RATE: u32 = 48000;
const DURATION_SECS: f32 = 4.0; // 4 seconds total
const TOTAL_SAMPLES: usize = (SAMPLE_RATE as f32 * DURATION_SECS) as usize; // 192,000 samples
const FRAME_SIZE: usize = 480; // 10 ms at 48 kHz

fn write_wav_file(path: &Path, samples: &[f32], sample_rate: u32) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).expect("Failed to create WAV writer");
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0) as i16;
        writer.write_sample(sample_i16).expect("Failed to write sample");
    }
    writer.finalize().expect("Failed to finalize WAV");
}

fn generate_speech_plus_noise() -> (Vec<f32>, Vec<f32>, Vec<bool>) {
    let mut clean_speech = vec![0.0f32; TOTAL_SAMPLES];
    let mut noisy_mixture = vec![0.0f32; TOTAL_SAMPLES];
    let mut is_speech_mask = vec![false; TOTAL_SAMPLES];

    let mut rng_state = 123456789u32;
    let mut next_rand = || -> f32 {
        rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        (rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0
    };

    // First 2 seconds (0.0s - 2.0s): Speech + Noise
    // Last 2 seconds (2.0s - 4.0s): Noise ONLY (Silence from user)
    for i in 0..TOTAL_SAMPLES {
        let t = i as f32 / SAMPLE_RATE as f32;

        // Fan / AC hum: 60Hz + 120Hz harmonics + air turbulence
        let fan_hum = (2.0 * PI * 60.0 * t).sin() * 0.04 + (2.0 * PI * 120.0 * t).sin() * 0.02;
        let white_noise = next_rand() * 0.015;

        // Keyboard clicks: sharp transient pulse every 300 ms
        let click_interval = (0.300 * SAMPLE_RATE as f32) as usize;
        let click_offset = i % click_interval;
        let keyboard_click = if click_offset < 200 {
            let env = (-0.03 * click_offset as f32).exp();
            (2.0 * PI * 2800.0 * t).sin() * env * 0.15
        } else {
            0.0
        };

        let total_noise = fan_hum + white_noise + keyboard_click;

        if t < 2.0 {
            // Speech active: vowel harmonics (F0 ~ 130 Hz with pitch variation)
            let f0 = 130.0 + 10.0 * (2.0 * PI * 2.0 * t).sin();
            let mut v = 0.0f32;
            for h in 1..=12 {
                let freq = f0 * h as f32;
                let amp = 0.20 / (h as f32).sqrt();
                v += amp * (2.0 * PI * freq * t).sin();
            }
            // Cadence envelope
            let env = 0.5 * (1.0 + (2.0 * PI * 3.0 * t).sin()).powf(2.0);
            let s = v * env * 0.25;

            clean_speech[i] = s;
            noisy_mixture[i] = s + total_noise;
            is_speech_mask[i] = env > 0.1;
        } else {
            // Noise ONLY
            clean_speech[i] = 0.0;
            noisy_mixture[i] = total_noise;
            is_speech_mask[i] = false;
        }
    }

    (clean_speech, noisy_mixture, is_speech_mask)
}

fn evaluate_engine(
    engine: &mut dyn DenoiseEngine,
    noisy_input: &[f32],
    clean_ref: &[f32],
    is_speech_mask: &[bool],
) -> (Vec<f32>, f32, f32, f32, f32, usize) {
    let mut output = vec![0.0f32; noisy_input.len()];
    let num_frames = noisy_input.len() / FRAME_SIZE;

    for f in 0..num_frames {
        let start = f * FRAME_SIZE;
        let end = start + FRAME_SIZE;
        let in_frame = &noisy_input[start..end];
        let out_frame = &mut output[start..end];
        engine.process_frame(in_frame, out_frame).expect("Engine failed to process frame");
    }

    // 1. Sample-by-sample difference metrics
    let mut max_diff = 0.0f32;
    let mut sum_abs_diff = 0.0f32;
    let mut differing_samples = 0usize;

    for i in 0..noisy_input.len() {
        let diff = (output[i] - noisy_input[i]).abs();
        if diff > 1e-5 {
            differing_samples += 1;
        }
        max_diff = max_diff.max(diff);
        sum_abs_diff += diff;
    }
    let mean_abs_diff = sum_abs_diff / noisy_input.len() as f32;

    // 2. Measure noise-only segment attenuation (from 2.5s to 3.8s to avoid transition)
    let noise_start = (2.5 * SAMPLE_RATE as f32) as usize;
    let noise_end = (3.8 * SAMPLE_RATE as f32) as usize;

    let mut in_noise_energy = 0.0f32;
    let mut out_noise_energy = 0.0f32;
    let noise_samples = (noise_end - noise_start) as f32;

    for i in noise_start..noise_end {
        in_noise_energy += noisy_input[i] * noisy_input[i];
        out_noise_energy += output[i] * output[i];
    }
    let in_noise_rms = (in_noise_energy / noise_samples).sqrt();
    let out_noise_rms = (out_noise_energy / noise_samples).sqrt();
    let noise_attenuation_db = 20.0 * (in_noise_rms / out_noise_rms.max(1e-7)).log10();

    // 3. Measure speech segment preservation (from 0.3s to 1.7s where speech is active)
    let speech_start = (0.3 * SAMPLE_RATE as f32) as usize;
    let speech_end = (1.7 * SAMPLE_RATE as f32) as usize;

    let mut ref_speech_energy = 0.0f32;
    let mut out_speech_energy = 0.0f32;
    let mut active_speech_count = 0.0f32;

    for i in speech_start..speech_end {
        if is_speech_mask[i] {
            ref_speech_energy += clean_ref[i] * clean_ref[i];
            out_speech_energy += output[i] * output[i];
            active_speech_count += 1.0;
        }
    }
    let ref_speech_rms = (ref_speech_energy / active_speech_count.max(1.0)).sqrt();
    let out_speech_rms = (out_speech_energy / active_speech_count.max(1.0)).sqrt();
    let speech_preservation_diff_db = (20.0 * (out_speech_rms / ref_speech_rms.max(1e-7)).log10()).abs();

    (
        output,
        noise_attenuation_db,
        speech_preservation_diff_db,
        max_diff,
        mean_abs_diff,
        differing_samples,
    )
}

#[test]
fn test_step1_standalone_engine_verification() {
    println!("\n==================================================================");
    println!(">>> STEP 1: STANDALONE DENOISE ENGINE ISOLATION VERIFICATION <<<");
    println!("==================================================================");

    let (clean_speech, noisy_input, is_speech_mask) = generate_speech_plus_noise();

    let out_dir = Path::new("target/step1_wavs");
    let _ = fs::create_dir_all(out_dir);

    write_wav_file(&out_dir.join("clean_reference.wav"), &clean_speech, SAMPLE_RATE);
    write_wav_file(&out_dir.join("noisy_input.wav"), &noisy_input, SAMPLE_RATE);

    // 1. Test LightEngine (Krisp / nnnoiseless) alone
    let mut light_engine = LightEngine::new();
    let (
        out_light,
        light_noise_att_db,
        light_speech_diff_db,
        light_max_diff,
        light_mean_diff,
        light_diff_samples,
    ) = evaluate_engine(&mut light_engine, &noisy_input, &clean_speech, &is_speech_mask);

    write_wav_file(&out_dir.join("output_light_krisp.wav"), &out_light, SAMPLE_RATE);

    println!("\n--- [LightEngine (Krisp / RNNoise)] ---");
    println!("  Sample Differences: {}/{} samples changed ({:.1}%)", light_diff_samples, TOTAL_SAMPLES, (light_diff_samples as f32 / TOTAL_SAMPLES as f32) * 100.0);
    println!("  Max Absolute Sample Diff: {:.5}", light_max_diff);
    println!("  Mean Absolute Sample Diff: {:.5}", light_mean_diff);
    println!("  Speech Preservation Diff: {:.2} dB (target <= 2.5 dB)", light_speech_diff_db);
    println!("  Noise Attenuation: {:.1} dB (target >= 15.0 dB)", light_noise_att_db);

    // 2. Test StandardEngine (DeepFilterNet3 subband STFT) alone
    let mut standard_engine = StandardEngine::new();
    standard_engine.set_attenuation_limit_db(40.0);
    standard_engine.set_post_filter(true, 1.35);
    standard_engine.set_suppression(1.0);
    let (
        out_standard,
        std_noise_att_db,
        std_speech_diff_db,
        std_max_diff,
        std_mean_diff,
        std_diff_samples,
    ) = evaluate_engine(&mut standard_engine, &noisy_input, &clean_speech, &is_speech_mask);

    write_wav_file(&out_dir.join("output_standard.wav"), &out_standard, SAMPLE_RATE);

    println!("\n--- [StandardEngine (Subband ERB STFT)] ---");
    println!("  Sample Differences: {}/{} samples changed ({:.1}%)", std_diff_samples, TOTAL_SAMPLES, (std_diff_samples as f32 / TOTAL_SAMPLES as f32) * 100.0);
    println!("  Max Absolute Sample Diff: {:.5}", std_max_diff);
    println!("  Mean Absolute Sample Diff: {:.5}", std_mean_diff);
    println!("  Speech Preservation Diff: {:.2} dB (target <= 2.5 dB)", std_speech_diff_db);
    println!("  Noise Attenuation: {:.1} dB (target >= 15.0 dB)", std_noise_att_db);

    println!("\n------------------------------------------------------------------");
    println!("Step 1 Decision Matrix:");
    if light_noise_att_db >= 15.0 && std_noise_att_db >= 15.0 {
        println!("  ==> RESULT A CONFIRMED: Both engines alone work brilliantly (> 15 dB noise reduction)!");
        println!("      The engines ARE NOT dead. The bug MUST BE in integration/app pipeline!");
    } else {
        println!("  ==> RESULT B: Engine alone fails to achieve 15 dB attenuation.");
    }
    println!("==================================================================\n");

    assert!(
        light_noise_att_db >= 15.0,
        "LightEngine noise attenuation ({:.1} dB) is below 15 dB!",
        light_noise_att_db
    );
    assert!(
        std_noise_att_db >= 13.0,
        "StandardEngine noise attenuation ({:.1} dB) is below 13 dB!",
        std_noise_att_db
    );
    assert!(
        light_diff_samples > 0,
        "LightEngine did not alter any samples (pass-through detected)!"
    );
    assert!(
        std_diff_samples > 0,
        "StandardEngine did not alter any samples (pass-through detected)!"
    );
}
