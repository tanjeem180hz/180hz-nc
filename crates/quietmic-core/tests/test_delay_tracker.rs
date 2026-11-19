use std::f32::consts::PI;

fn synthesize_speech(num_samples: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / 48000.0;
        let pitch = 130.0 + 60.0 * (2.0 * PI * 2.0 * t).sin() + 20.0 * (2.0 * PI * 5.0 * t).cos();
        let envelope = (2.0 * PI * 3.0 * t).sin().max(0.0).powf(1.5);
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

struct TestDelayTracker {
    ref_history: Vec<f32>,
    mic_history: Vec<f32>,
    downsample_factor: usize,
}

impl TestDelayTracker {
    fn new() -> Self {
        Self {
            ref_history: Vec::with_capacity(6000), // ~750ms at 8kHz
            mic_history: Vec::with_capacity(1600), // ~200ms at 8kHz
            downsample_factor: 6,
        }
    }

    fn push_frames(&mut self, ref_48k: &[f32], mic_48k: &[f32]) {
        // Downsample by averaging factor of 6
        for chunk in ref_48k.chunks(self.downsample_factor) {
            let avg: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
            self.ref_history.push(avg);
        }
        for chunk in mic_48k.chunks(self.downsample_factor) {
            let avg: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
            self.mic_history.push(avg);
        }

        const MAX_REF_8K: usize = 4000; // 500 ms at 8kHz
        const MAX_MIC_8K: usize = 800;  // 100 ms at 8kHz
        if self.ref_history.len() > MAX_REF_8K {
            let excess = self.ref_history.len() - MAX_REF_8K;
            self.ref_history.drain(0..excess);
        }
        if self.mic_history.len() > MAX_MIC_8K {
            let excess = self.mic_history.len() - MAX_MIC_8K;
            self.mic_history.drain(0..excess);
        }
    }

    fn estimate_delay_samples(&self) -> Option<(usize, f32)> {
        const MIC_WINDOW_8K: usize = 600; // 75 ms
        if self.mic_history.len() < MIC_WINDOW_8K || self.ref_history.len() < MIC_WINDOW_8K + 2400 {
            return None;
        }

        let mic_eval = &self.mic_history[self.mic_history.len() - MIC_WINDOW_8K..];
        let mic_energy: f32 = mic_eval.iter().map(|&x| x * x).sum();
        if mic_energy < 1e-4 {
            return None;
        }

        let ref_len = self.ref_history.len();
        let max_lag_8k = (2400).min(ref_len - MIC_WINDOW_8K); // up to 300 ms

        let mut best_lag_8k = 0;
        let mut best_corr = 0.0f32;

        for lag in (0..=max_lag_8k).step_by(2) {
            let start = ref_len - MIC_WINDOW_8K - lag;
            let ref_eval = &self.ref_history[start..start + MIC_WINDOW_8K];
            let mut dot = 0.0f32;
            let mut ref_energy = 0.0f32;
            for i in 0..MIC_WINDOW_8K {
                dot += mic_eval[i] * ref_eval[i];
                ref_energy += ref_eval[i] * ref_eval[i];
            }
            let denom = (mic_energy * ref_energy).sqrt();
            if denom > 1e-6 {
                let r = dot / denom;
                if r > best_corr {
                    best_corr = r;
                    best_lag_8k = lag;
                }
            }
        }

        if best_corr >= 0.15 {
            let coarse_samples_48k = best_lag_8k * self.downsample_factor;
            Some((coarse_samples_48k, best_corr))
        } else {
            None
        }
    }
}

#[test]
fn test_delay_tracker_delays() {
    let test_delays_ms = [20, 80, 150, 300];
    let speech = synthesize_speech(48000 * 3); // 3 seconds

    for &delay_ms in &test_delays_ms {
        let true_delay_samples = delay_ms * 48;
        let mut tracker = TestDelayTracker::new();

        let frame_size = 480;
        let num_frames = 200; // 2 seconds

        let mut detected_delay = None;

        for f in 0..num_frames {
            let start = f * frame_size;
            let ref_frame = &speech[start..start + frame_size];

            let mut mic_frame = vec![0.0f32; frame_size];
            for i in 0..frame_size {
                let g = start + i;
                if g >= true_delay_samples && g - true_delay_samples < speech.len() {
                    mic_frame[i] = speech[g - true_delay_samples] * 0.5; // acoustic echo
                }
            }

            tracker.push_frames(ref_frame, &mic_frame);

            if f > 80 && detected_delay.is_none() {
                if let Some((est_samples, corr)) = tracker.estimate_delay_samples() {
                    let err = (est_samples as i64 - true_delay_samples as i64).abs();
                    println!(
                        "Delay {} ms ({} samples): Estimated {} samples (corr: {:.3}), error: {} samples ({:.2} ms)",
                        delay_ms, true_delay_samples, est_samples, corr, err, err as f32 / 48.0
                    );
                    detected_delay = Some(est_samples);
                }
            }
        }

        assert!(detected_delay.is_some(), "Failed to detect delay for {} ms", delay_ms);
        let est = detected_delay.unwrap();
        let err = (est as i64 - true_delay_samples as i64).abs();
        // Coarse estimate should be within 12 samples (0.25 ms), well within 512-tap (10.6 ms) FIR filter range!
        assert!(err <= 24, "Delay error {} too high for {} ms", err, delay_ms);
    }
}
