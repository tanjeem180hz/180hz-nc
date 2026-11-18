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
}

impl ResidualSuppressor {
    pub fn new() -> Self {
        Self {
            running_noise_floor: 1e-4,
            running_speech_level: 1e-2,
            suppression_gain: 1.0,
            prev_sample: 0.0,
            enabled: true,
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

    pub fn reset(&mut self) {
        self.running_noise_floor = 1e-4;
        self.running_speech_level = 1e-2;
        self.suppression_gain = 1.0;
        self.prev_sample = 0.0;
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
        let is_voiced_speech = low_e > high_e * 1.2 && crest_factor < 4.0;

        // Update running noise floor and speech level trackers:
        // Adapt noise floor during non-voiced intervals (noise/hiss), freeze during voiced speech
        if !is_voiced_speech || frame_rms < self.running_noise_floor * 1.5 {
            self.running_noise_floor += 0.05 * (frame_rms - self.running_noise_floor);
        } else if frame_rms > self.running_noise_floor * 2.5 {
            self.running_speech_level += 0.05 * (frame_rms - self.running_speech_level);
        }
        self.running_noise_floor = self.running_noise_floor.clamp(1e-6, 0.05);
        self.running_speech_level = self.running_speech_level.clamp(0.01, 1.0);

        let snr = frame_rms / self.running_noise_floor;

        // Smooth post-filter gain
        let target_gain = if snr < 1.5 {
            0.05 // 26 dB residual suppression during quiet intervals
        } else if snr < 3.0 {
            0.50 // Soft blend in transitional vowels
        } else {
            1.0 // Full 100% natural speech preservation
        };

        let release_rate = if target_gain > self.suppression_gain {
            0.08 // Fast recovery so speech consonants are never muffled
        } else {
            0.05
        };

        for s in frame.iter_mut() {
            let in_val = *s;
            let abs_val = in_val.abs();
            let deriv = (in_val - self.prev_sample).abs();
            self.prev_sample = in_val;

            self.suppression_gain += release_rate * (target_gain - self.suppression_gain);

            // Fast clamp on sharp mechanical click impulses ONLY during quiet intervals / non-speech.
            // Never clamp active voiced speech waveforms to prevent cracking or clipping loud voice!
            let is_click_impulse = !is_voiced_speech && (deriv > 0.12) && (abs_val > 0.15);
            if is_click_impulse {
                *s = in_val.clamp(-0.10, 0.10);
            } else {
                *s = in_val * self.suppression_gain;
            }
        }
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
