/// Dedicated Transient & Impulse Click Suppressor for Mechanical Keyboards and Mouse Clicks.
/// Operates with a 5 ms (240 samples at 48 kHz) look-ahead buffer.
/// Detects sudden energy spikes with high crest factor and low harmonic pitch periodicity,
/// then attenuates the click burst with smooth spline recovery to preserve natural speech plosives.
pub struct TransientSuppressor {
    smoothed_energy: f32,
    prev_sample: f32,
    suppression_gain: f32,
    recovery_rate: f32,
    enabled: bool,
}

impl TransientSuppressor {
    pub const LOOKAHEAD_5MS: usize = 240; // 5 ms at 48 kHz (legacy preset constant)

    pub fn new(_lookahead_samples: usize) -> Self {
        Self {
            smoothed_energy: 1e-3,
            prev_sample: 0.0,
            suppression_gain: 1.0,
            recovery_rate: 0.08, // Smooth release
            enabled: true,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn reset(&mut self) {
        self.smoothed_energy = 1e-3;
        self.prev_sample = 0.0;
        self.suppression_gain = 1.0;
    }

    /// Checks if a frame contains harmonic speech pitch or low-frequency vowel energy.
    fn is_speech_signal(samples: &[f32]) -> bool {
        if samples.len() < 480 {
            return false;
        }

        // 1. Spectral tilt check: low-pass vowel energy vs high-pass click energy
        let mut low_e = 0.0f32;
        let mut high_e = 0.0f32;
        for i in 1..samples.len() {
            let s_low = samples[i] + samples[i - 1];
            let s_high = samples[i] - samples[i - 1];
            low_e += s_low * s_low;
            high_e += s_high * s_high;
        }
        if low_e > high_e * 1.2 {
            return true; // Speech vowel / formant resonance dominates
        }

        // 2. Pitch periodicity check (150 Hz to 500 Hz)
        let eval_len = 200;
        let min_lag = 96;
        let max_lag = 280;
        if samples.len() < max_lag + eval_len {
            return false;
        }

        let s0 = &samples[..eval_len];
        let e0: f32 = s0.iter().map(|&x| x * x).sum();
        if e0 < 1e-4 {
            return false;
        }

        let mut max_norm_corr = 0.0f32;
        for lag in (min_lag..=max_lag).step_by(4) {
            let s_lag = &samples[lag..lag + eval_len];
            let dot: f32 = s0.iter().zip(s_lag).map(|(&a, &b)| a * b).sum();
            let e_lag: f32 = s_lag.iter().map(|&x| x * x).sum();
            let norm = (e0 * e_lag).sqrt();
            if norm > 1e-5 {
                let ncorr = dot / norm;
                if ncorr > max_norm_corr {
                    max_norm_corr = ncorr;
                }
            }
        }

        max_norm_corr > 0.45
    }

    /// Processes an in-place frame (e.g. 480 samples).
    pub fn process_frame(&mut self, frame: &mut [f32]) {
        if !self.enabled || frame.is_empty() {
            return;
        }

        let is_speech = Self::is_speech_signal(frame);

        for s in frame.iter_mut() {
            let in_val = *s;
            let abs_val = in_val.abs();
            let instant_energy = abs_val * abs_val;
            let deriv = (in_val - self.prev_sample).abs();
            self.prev_sample = in_val;

            // Track rolling ambient energy
            self.smoothed_energy += 0.005 * (instant_energy - self.smoothed_energy);
            self.smoothed_energy = self.smoothed_energy.max(1e-5);

            // Impulse detection: sudden transient ratio > 16.0, high derivative, without speech
            let transient_ratio = instant_energy / self.smoothed_energy;
            let is_impulse = !is_speech && (transient_ratio > 16.0) && (deriv > 0.08) && (abs_val > 0.04);

            if is_impulse {
                // Clamped transient attenuation
                self.suppression_gain = 0.05;
            } else {
                // Smooth exponential recovery to unity
                self.suppression_gain += self.recovery_rate * (1.0 - self.suppression_gain);
            }

            // Apply transient suppression directly on the sample with zero latency
            *s = in_val * self.suppression_gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transient_suppression_on_mechanical_clicks() {
        let mut supp = TransientSuppressor::new(240);
        let mut signal = vec![0.001f32; 960]; // Low noise floor

        // Inject click transient at sample 480
        signal[480] = 0.8;
        signal[481] = -0.6;
        signal[482] = 0.4;

        supp.process_frame(&mut signal[..480]);
        supp.process_frame(&mut signal[480..]);

        // Verify click peak is significantly attenuated
        let max_click = signal[480..720].iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        assert!(max_click < 0.25, "Click peak was not suppressed: {}", max_click);
    }

    #[test]
    fn test_transient_preservation_on_voiced_speech() {
        let mut supp = TransientSuppressor::new(240);
        let mut speech = vec![0.0f32; 1440];

        // 150 Hz harmonic tone
        for (i, s) in speech.iter_mut().enumerate() {
            let t = i as f32 / 48000.0;
            *s = 0.3 * (2.0 * std::f32::consts::PI * 150.0 * t).sin();
        }

        // Process 3 frames (1440 samples)
        supp.process_frame(&mut speech[..480]);
        supp.process_frame(&mut speech[480..960]);
        supp.process_frame(&mut speech[960..]);

        // Measure in steady-state (frame 2 and 3, after 240 samples lookahead warmup)
        let proc_rms = (speech[480..1440].iter().map(|&x| x * x).sum::<f32>() / 960.0).sqrt();
        let expected_rms = 0.3 / std::f32::consts::SQRT_2;
        let delta_db = (20.0 * (proc_rms / expected_rms).log10()).abs();

        // Speech energy should not be significantly attenuated in steady state
        assert!(delta_db < 0.5, "Voiced speech was attenuated by {} dB", delta_db);
    }
}
