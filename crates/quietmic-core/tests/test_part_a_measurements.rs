use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};
use std::f32::consts::PI;

/// Deterministic PRNG for test repeatability
struct SimpleRng(u64);
impl SimpleRng {
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 32) as u32 as f32) / (u32::MAX as f32)
    }
    fn next_bipolar(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

/// Generates a test mixture with:
/// - 0.0s to 2.0s: Active Speech + Combined Noise (Keyboard clicks + Fan/AC + Background Talker)
/// - 2.0s to 4.0s: Combined Noise ONLY (no speech)
fn generate_test_signal(target_snr_db: f32) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let sample_rate = 48000;
    let total_samples = 4 * sample_rate; // 4 seconds
    let speech_samples = 2 * sample_rate; // 2 seconds speech

    let mut clean_speech = vec![0.0f32; total_samples];
    let mut noise_only = vec![0.0f32; total_samples];
    let mut noisy_mixture = vec![0.0f32; total_samples];

    let mut rng = SimpleRng(0x180_AEC_FE01);

    // 1. Generate clean speech with realistic harmonic structure (F0 = 135 Hz)
    for i in 0..speech_samples {
        let t = i as f32 / sample_rate as f32;
        let f0 = 135.0 + 8.0 * (2.0 * PI * 1.5 * t).sin();
        let mut s = 0.0f32;
        for h in 1..=16 {
            let freq = f0 * h as f32;
            let amp = 0.25 / (h as f32).sqrt();
            s += amp * (2.0 * PI * freq * t).sin();
        }
        // Cadence envelope (words and syllables)
        let env = 0.5 * (1.0 + (2.0 * PI * 3.5 * t).sin()).powf(2.0);
        clean_speech[i] = s * env * 0.3;
    }

    // 2. Generate combined noise:
    // (a) Fan + AC hum (60 Hz + 120 Hz + lowpass broadband rumble)
    // (b) Keyboard typing clicks (sharp bursts every 280 ms)
    // (c) Background talker (distant female voice F0 = 240 Hz)
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let fan = 0.25 * (2.0 * PI * 60.0 * t).sin()
            + 0.15 * (2.0 * PI * 120.0 * t).sin()
            + 0.10 * (2.0 * PI * 180.0 * t).sin()
            + 0.08 * rng.next_bipolar();

        // Keyboard clicks: sharp 4 ms transient impulse every 280 ms
        let click_interval = (0.280 * sample_rate as f32) as usize;
        let click_idx = i % click_interval;
        let keyboard = if click_idx < 192 {
            let decay = (-18.0 * (click_idx as f32 / 192.0)).exp();
            let click_osc = (2.0 * PI * 3200.0 * (click_idx as f32 / sample_rate as f32)).sin();
            (click_osc + 0.4 * rng.next_bipolar()) * decay * 0.4
        } else {
            0.0
        };

        // Distant background secondary talker (F0 = 240 Hz, cadence 4.2 Hz)
        let talker_f0 = 240.0;
        let talker_env = 0.5 * (1.0 + (2.0 * PI * 4.2 * t).sin()).powi(2);
        let bg_talker = 0.15 * (2.0 * PI * talker_f0 * t).sin() * talker_env;

        noise_only[i] = fan + keyboard + bg_talker;
    }

    // Scale noise to desired target SNR relative to active speech
    let speech_energy: f32 = clean_speech[..speech_samples].iter().map(|&x| x * x).sum();
    let speech_rms = (speech_energy / speech_samples as f32).sqrt();

    let noise_energy: f32 = noise_only[..speech_samples].iter().map(|&x| x * x).sum();
    let noise_rms = (noise_energy / speech_samples as f32).sqrt();

    let target_noise_rms = speech_rms / (10.0f32.powf(target_snr_db / 20.0));
    let scale = target_noise_rms / noise_rms.max(1e-6);

    for i in 0..total_samples {
        noise_only[i] *= scale;
        noisy_mixture[i] = clean_speech[i] + noise_only[i];
    }

    (clean_speech, noise_only, noisy_mixture)
}

fn evaluate_pipeline_segments(
    pipeline: &mut AudioPipeline,
    clean_speech: &[f32],
    noise_only: &[f32],
    noisy_mixture: &[f32],
) -> (f32, f32) {
    let frame_size = 480;
    let num_frames = noisy_mixture.len() / frame_size;
    let mut cleaned = Vec::with_capacity(noisy_mixture.len());
    let mut scratch_out = Vec::with_capacity(490);

    for f in 0..num_frames {
        let start = f * frame_size;
        let end = start + frame_size;
        let in_frame = &noisy_mixture[start..end];
        let _ = pipeline.process_frame_internal(in_frame, &mut scratch_out, 960);
        cleaned.extend_from_slice(&scratch_out);
    }

    // 1. Noise-only segment (2.5s to 3.8s, to avoid boundary effects)
    let n_start = 2 * 48000 + 24000; // 2.5s
    let n_end = 3 * 48000 + 38400;   // 3.8s
    let in_noise_rms = (noise_only[n_start..n_end].iter().map(|&x| x * x).sum::<f32>() / (n_end - n_start) as f32).sqrt();
    let out_noise_rms = (cleaned[n_start..n_end].iter().map(|&x| x * x).sum::<f32>() / (n_end - n_start) as f32).sqrt();
    let noise_only_reduction_db = 20.0 * (in_noise_rms / out_noise_rms.max(1e-7)).log10();

    // 2. Speech-active segment (0.3s to 1.7s)
    // Pipeline has algorithmic delay (10-15 ms). Find optimal lag to align clean speech.
    let s_start = 14400; // 0.3s
    let s_end = 81600;   // 1.7s
    let eval_len = s_end - s_start;

    // Cross-correlation lag search
    let mut best_lag = 0;
    let mut max_corr = f32::NEG_INFINITY;
    for lag in 0..=1920 {
        let mut corr = 0.0f32;
        for i in 0..eval_len.min(3000) {
            corr += clean_speech[s_start + i] * cleaned[s_start + lag + i];
        }
        if corr > max_corr {
            max_corr = corr;
            best_lag = lag;
        }
    }

    // Compute scale factor alpha to account for linear gain variation on speech
    let mut dot = 0.0f32;
    let mut clean_norm_sq = 0.0f32;
    for i in 0..eval_len {
        let true_speech = clean_speech[s_start + i];
        let processed = cleaned[s_start + best_lag + i];
        dot += processed * true_speech;
        clean_norm_sq += true_speech * true_speech;
    }
    let alpha = (dot / clean_norm_sq.max(1e-9)).clamp(0.5, 1.5);

    // Compute residual noise during active speech:
    // e[t] = cleaned[t + lag] - alpha * clean_speech[t]
    let mut residual_energy = 0.0f32;
    let mut input_noise_energy = 0.0f32;
    for i in 0..eval_len {
        let true_speech = clean_speech[s_start + i];
        let true_noise = noise_only[s_start + i];
        let processed = cleaned[s_start + best_lag + i];
        let err = processed - alpha * true_speech;
        residual_energy += err * err;
        input_noise_energy += true_noise * true_noise;
    }

    let in_speech_noise_rms = (input_noise_energy / eval_len as f32).sqrt();
    let out_speech_residual_rms = (residual_energy / eval_len as f32).sqrt();
    let speech_active_reduction_db = 20.0 * (in_speech_noise_rms / out_speech_residual_rms.max(1e-7)).log10();

    println!("    [DEBUG] best_lag = {}, alpha = {:.4}, in_noise_rms = {:.5}, out_res_rms = {:.5}",
        best_lag, alpha, in_speech_noise_rms, out_speech_residual_rms);

    (noise_only_reduction_db, speech_active_reduction_db)
}

#[test]
fn test_part_a_measure_noise_only_vs_speech_active() {
    println!("\n==========================================================================");
    println!(">>> PART A: RESIDUAL NOISE DIAGNOSTIC (NOISE-ONLY vs SPEECH-ACTIVE) <<<");
    println!("==========================================================================");

    for &snr in &[0.0f32, 5.0f32, 10.0f32] {
        let (clean_speech, noise_only, noisy_mixture) = generate_test_signal(snr);

        // 1. Test Light Mode (Krisp / RNNoise)
        let mut light_pipe = AudioPipeline::new(48000, 48000).unwrap();
        light_pipe.set_mode(EngineMode::Light);
        light_pipe.set_strength(1.0);
        let (light_no_nr, light_sp_nr) = evaluate_pipeline_segments(&mut light_pipe, &clean_speech, &noise_only, &noisy_mixture);

        // 2. Test Standard Mode (DeepFilterNet3 subband)
        let mut std_pipe = AudioPipeline::new(48000, 48000).unwrap();
        std_pipe.set_mode(EngineMode::Standard);
        std_pipe.set_strength(1.0);
        let (std_no_nr, std_sp_nr) = evaluate_pipeline_segments(&mut std_pipe, &clean_speech, &noise_only, &noisy_mixture);

        // 3. Test Max Quality Mode (Dual-Stage Neural + ERB Wiener)
        let mut max_pipe = AudioPipeline::new(48000, 48000).unwrap();
        max_pipe.set_mode(EngineMode::MaxQuality);
        max_pipe.set_strength(1.0);
        let (max_no_nr, max_sp_nr) = evaluate_pipeline_segments(&mut max_pipe, &clean_speech, &noise_only, &noisy_mixture);

        println!("[SNR = {:>2.0} dB]  Light Mode      => Noise-Only Red: {:>5.1} dB | Speech-Active Red: {:>5.1} dB", snr, light_no_nr, light_sp_nr);
        println!("[SNR = {:>2.0} dB]  Standard Mode   => Noise-Only Red: {:>5.1} dB | Speech-Active Red: {:>5.1} dB", snr, std_no_nr, std_sp_nr);
        println!("[SNR = {:>2.0} dB]  MaxQuality Mode => Noise-Only Red: {:>5.1} dB | Speech-Active Red: {:>5.1} dB", snr, max_no_nr, max_sp_nr);
        println!("--------------------------------------------------------------------------");
    }
}
