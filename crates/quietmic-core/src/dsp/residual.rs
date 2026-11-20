/// Second-Stage Residual Noise & Transient Suppressor.
/// Runs AFTER the main denoise engine (Step 5b in AudioPipeline) to eliminate:
/// 1. Residual keyboard click bursts and mouse tap impulses that slipped through neural inference.
/// 2. Residual inter-harmonic hiss and stationary fan/AC noise floor in speech valleys.
///
/// Operates with 0 additional algorithmic latency (zero buffer delay) to preserve
/// ultra-fast speech onset and strict compliance with the < 30 ms latency budget.
pub struct ResidualSuppressor {
    running_noise_floor: f32,
    running_speech_level: f32,
    suppression_gain: f32,
    prev_sample: f32,
    enabled: bool,
    strength: f32,
}

impl ResidualSuppressor {
    pub fn new() -> Self {
        Self {
            running_noise_floor: 1e-4,
            running_speech_level: 1e-2,
            suppression_gain: 1.0,
            prev_sample: 0.0,
            enabled: true,
            strength: 1.0,
        }
    }

    pub fn default_preset() -> Self {
        Self::new()
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_strength(&mut self, strength: f32) {
        self.strength = strength.clamp(0.0, 1.0);
    }

    pub fn strength(&self) -> f32 {
        self.strength
    }

    pub fn reset(&mut self) {
        self.running_noise_floor = 1e-4;
        self.running_speech_level = 1e-2;
        self.suppression_gain = 1.0;
        self.prev_sample = 0.0;
        self.strength = 1.0;
    }

    /// Processes a 10 ms frame (480 samples) in-place with zero latency.
    pub fn process_frame(&mut self, frame: &mut [f32]) {
        if !self.enabled || frame.is_empty() {
            return;
        }

        // Measure frame RMS and peak
        let mut sum_sq = 0.0f32;
        let mut peak = 0.0f32;
        for &s in frame.iter() {
            let abs = s.abs();
            if abs > peak {
                peak = abs;
            }
            sum_sq += abs * abs;
        }
        let frame_rms = (sum_sq / frame.len() as f32).sqrt().max(1e-6);

        let mut low_e = 0.0f32;
        let mut high_e = 0.0f32;
        for i in 1..frame.len() {
            let s_low = frame[i] + frame[i - 1];
            let s_high = frame[i] - frame[i - 1];
            low_e += s_low * s_low;
            high_e += s_high * s_high;
        }
        let crest_factor = peak / frame_rms;

        // Pitch periodicity check in speech fundamental range (120 Hz to 400 Hz)
        let mut max_norm_corr = 0.0f32;
        if frame.len() >= 360 {
            let eval_len = 160;
            let s0 = &frame[..eval_len];
            let e0: f32 = s0.iter().map(|&x| x * x).sum();
            if e0 > 1e-5 {
                for lag in (120..=300).step_by(6) {
                    if lag + eval_len <= frame.len() {
                        let s_lag = &frame[lag..lag + eval_len];
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
                }
            }
        }

        // Voiced speech requires harmonic pitch correlation or dominant formant resonance with speech-level energy
        let is_voiced_speech = (max_norm_corr > 0.38 || (frame_rms > 0.03 && low_e > high_e * 2.0)) && crest_factor < 5.0 && frame_rms > 0.003;

        // Update running noise floor and speech level trackers:
        // Adapt noise floor during non-voiced intervals (noise/hiss), freeze during voiced speech
        if !is_voiced_speech {
            self.running_noise_floor += 0.05 * (frame_rms - self.running_noise_floor);
        } else if frame_rms > self.running_noise_floor * 2.0 {
            self.running_speech_level += 0.05 * (frame_rms - self.running_speech_level);
        }
        self.running_noise_floor = self.running_noise_floor.clamp(1e-6, 0.05);
        self.running_speech_level = self.running_speech_level.clamp(0.01, 1.0);

        let snr = frame_rms / self.running_noise_floor;

        // Smooth post-filter gain
        let quiet_gain = 0.02 * (1.0 - self.strength * 0.85); // down to 0.003 (-50 dB)
        let target_gain = if is_voiced_speech || snr > 2.5 {
            1.0 // Full 100% natural speech preservation
        } else if snr < 1.5 {
            quiet_gain
        } else {
            0.50 * (1.0 - self.strength * 0.50) // Soft blend in transitional vowels
        };

        let release_rate = if target_gain > self.suppression_gain {
            0.15 // Fast recovery so speech consonants are never muffled
        } else {
            0.04
        };

        let click_deriv_thresh = 0.12 - 0.03 * self.strength;
        let click_abs_thresh = 0.15 - 0.05 * self.strength;
        let click_clamp = 0.10 * (1.0 - self.strength * 0.60);

        for s in frame.iter_mut() {
            let in_val = *s;
            let abs_val = in_val.abs();
            let deriv = (in_val - self.prev_sample).abs();
            self.prev_sample = in_val;

            self.suppression_gain += release_rate * (target_gain - self.suppression_gain);

            // Fast clamp on sharp mechanical click impulses ONLY during quiet intervals / non-speech.
            // Never clamp active voiced speech waveforms to prevent cracking or clipping loud voice!
            let is_click_impulse = !is_voiced_speech && (deriv > click_deriv_thresh) && (abs_val > click_abs_thresh);
            if is_click_impulse {
                *s = in_val.clamp(-click_clamp, click_clamp);
            } else {
                *s = in_val * self.suppression_gain;
            }
        }
    }
}

impl Default for ResidualSuppressor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_residual_suppressor_transient_clamping() {
        let mut res = ResidualSuppressor::new();
        let mut frame = vec![0.001f32; 480];
        // Inject residual click spike
        frame[240] = 0.6;
        frame[241] = -0.5;

        res.process_frame(&mut frame);

        // Verify spike was clamped
        assert!(frame[240].abs() <= 0.15, "Residual click was not clamped: got {}", frame[240]);
    }
}
