use crate::engine::pipeline::AudioPipeline;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Result report from the pipeline Self-Test diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfTestReport {
    pub passed: bool,
    pub noise_only_reduction_db: f32,
    pub speech_active_reduction_db: f32,
    pub target_noise_only_db: f32,
    pub target_speech_active_db: f32,
    pub summary: String,
}

/// Runs a self-contained pipeline test by injecting a standardized noisy signal
/// (speech + mechanical keyboard clicks + stationary fan hum) through the live pipeline.
/// PASS condition:
/// - Noise-only segment reduction >= 15.0 dB
/// - Speech-active segment noise reduction >= 8.0 dB
pub fn run_pipeline_self_test(pipeline: &mut AudioPipeline) -> SelfTestReport {
    let sample_rate = 48000;
    let total_samples = 3 * sample_rate; // 3 seconds
    let _speech_samples = sample_rate;    // 1 second speech (0.5s to 1.5s)

    let mut noisy_signal = vec![0.0f32; total_samples];
    let mut clean_speech = vec![0.0f32; total_samples];
    let mut noise_only = vec![0.0f32; total_samples];

    // 1. Generate clean speech with natural pitch harmonics (F0 = 150 Hz) between 0.5s and 1.5s
    let speech_start = sample_rate / 2;
    let speech_end = sample_rate * 3 / 2;
    for (idx, sample) in clean_speech[speech_start..speech_end].iter_mut().enumerate() {
        let i = speech_start + idx;
        let t = i as f32 / sample_rate as f32;
        let f0 = 150.0 + 10.0 * (2.0 * PI * 2.0 * t).sin();
        let mut s = 0.0f32;
        for h in 1..=12 {
            let freq = f0 * h as f32;
            let amp = 0.20 / (h as f32).sqrt();
            s += amp * (2.0 * PI * freq * t).sin();
        }
        let env = (PI * (t - 0.5)).sin(); // smooth half-sine envelope
        *sample = s * env * 0.35;
    }

    // 2. Generate noise (fan hum 60/120 Hz + sharp keyboard clicks every 300 ms + background hiss)
    let mut rng = 0xF1F0_5E1Fu32;
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let hiss = (rng as f32 / u32::MAX as f32 - 0.5) * 0.02;

        let fan = 0.03 * (2.0 * PI * 60.0 * t).sin()
            + 0.018 * (2.0 * PI * 120.0 * t).sin()
            + 0.010 * (2.0 * PI * 180.0 * t).sin()
            + hiss;

        let click_interval = (0.300 * sample_rate as f32) as usize;
        let click_idx = i % click_interval;
        let click = if click_idx < 144 {
            let decay = (-20.0 * (click_idx as f32 / 144.0)).exp();
            0.20 * (2.0 * PI * 3400.0 * (click_idx as f32 / sample_rate as f32)).sin() * decay
        } else {
            0.0
        };

        noise_only[i] = fan + click;
        noisy_signal[i] = clean_speech[i] + noise_only[i];
    }

    // 3. Process noisy signal frame by frame through the pipeline
    let frame_size = AudioPipeline::FRAME_SAMPLES;
    let num_frames = total_samples / frame_size;
    let mut cleaned = Vec::with_capacity(total_samples);
    let mut scratch_out = Vec::with_capacity(frame_size + 10);

    for f in 0..num_frames {
        let start = f * frame_size;
        let end = start + frame_size;
        let in_frame = &noisy_signal[start..end];
        let _ = pipeline.process_frame_internal(in_frame, &mut scratch_out, 960);
        cleaned.extend_from_slice(&scratch_out);
    }

    // 4. Measure Noise-only segment (2.0s to 2.8s)
    let n_start = 2 * sample_rate;
    let n_end = 2 * sample_rate + (0.8 * sample_rate as f32) as usize;
    let in_noise_rms = (noise_only[n_start..n_end].iter().map(|&x| x * x).sum::<f32>() / (n_end - n_start) as f32).sqrt();
    let out_noise_rms = (cleaned[n_start..n_end].iter().map(|&x| x * x).sum::<f32>() / (n_end - n_start) as f32).sqrt();
    let noise_only_reduction_db = (20.0 * (in_noise_rms / out_noise_rms.max(1e-7)).log10()).max(0.0);

    eprintln!("SELF_TEST: in_noise_rms={:.5}, out_noise_rms={:.5}, reduction={:.2} dB", in_noise_rms, out_noise_rms, noise_only_reduction_db);

    // 5. Measure Speech-active inter-harmonic noise reduction (0.7s to 1.3s)
    // Pipeline has algorithmic delay (15 ms = 720 samples).
    let lag = 720;
    let s_start = (0.7 * sample_rate as f32) as usize;
    let s_end = (1.3 * sample_rate as f32) as usize;
    let eval_len = s_end - s_start;

    // Measure high-frequency and valley noise energy during speech
    // High-pass above 2200 Hz contains no speech harmonics in our test signal, purely noise!
    let mut in_hf_noise_e = 0.0f32;
    let mut out_hf_noise_e = 0.0f32;

    for i in 1..eval_len {
        let idx = s_start + i;
        let out_idx = (idx + lag).min(cleaned.len() - 1);

        // Simple 2nd difference high-pass filter (cuts low frequencies < 2 kHz, measures click and hiss noise)
        let in_diff = noisy_signal[idx] - 2.0 * noisy_signal[idx - 1] + noisy_signal[idx.saturating_sub(2)];
        let out_diff = cleaned[out_idx] - 2.0 * cleaned[out_idx - 1] + cleaned[out_idx.saturating_sub(2)];

        in_hf_noise_e += in_diff * in_diff;
        out_hf_noise_e += out_diff * out_diff;
    }

    let in_hf_rms = (in_hf_noise_e / eval_len as f32).sqrt().max(1e-7);
    let out_hf_rms = (out_hf_noise_e / eval_len as f32).sqrt().max(1e-7);
    let speech_active_reduction_db = (20.0 * (in_hf_rms / out_hf_rms).log10()).max(0.0);

    let target_noise_only_db = 15.0;
    let target_speech_active_db = 8.0;

    let passed = noise_only_reduction_db >= target_noise_only_db
        && speech_active_reduction_db >= target_speech_active_db;

    let summary = if passed {
        format!(
            "PASS: Noise suppression verified. Noise-only reduction: {:.1} dB (target >= {:.1} dB), Speech-active reduction: {:.1} dB (target >= {:.1} dB).",
            noise_only_reduction_db, target_noise_only_db, speech_active_reduction_db, target_speech_active_db
        )
    } else {
        format!(
            "FAIL: Noise-only reduction: {:.1} dB (target >= {:.1} dB), Speech-active reduction: {:.1} dB (target >= {:.1} dB).",
            noise_only_reduction_db, target_noise_only_db, speech_active_reduction_db, target_speech_active_db
        )
    };

    SelfTestReport {
        passed,
        noise_only_reduction_db,
        speech_active_reduction_db,
        target_noise_only_db,
        target_speech_active_db,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::pipeline::EngineMode;

    #[test]
    fn test_pipeline_self_test_passes_criteria() {
        let mut pipe = AudioPipeline::new(48000, 48000).unwrap();
        pipe.set_mode(EngineMode::Standard);
        pipe.set_strength(1.0);

        let report = run_pipeline_self_test(&mut pipe);
        println!("\n>>> SELF TEST REPORT <<<\n{}", report.summary);
        assert!(
            report.passed,
            "Pipeline self-test failed to meet >=15dB noise-only and >=8dB speech-active criteria: {:?}",
            report
        );
    }
}
