/// Acoustic Feedback (Howling) Detector for real-time live monitoring safety.
/// Automatically detects runaway acoustic feedback loops caused by microphone-to-speaker
/// coupling when headphones are not used. Instantly triggers to mute monitor output before
/// hearing damage occurs.
#[derive(Debug, Clone)]
pub struct HowlingDetector {
    sample_rate: f32,
    energy_threshold: f32,      // Minimum RMS energy to consider (e.g. -24 dBFS)
    consecutive_howl_frames: usize,
    howl_trigger_frames: usize,  // Consecutive frames needed to declare feedback (e.g. 6 frames = 60 ms)
    prev_period: f32,
    triggered: bool,
}

impl HowlingDetector {
    /// Creates a new HowlingDetector.
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sampling rate (e.g. 48000.0)
    /// * `threshold_dbfs` - RMS energy threshold for howling detection (e.g. -24.0 dBFS)
    /// * `trigger_ms` - Duration of sustained feedback required to trigger (e.g. 60.0 ms)
    pub fn new(sample_rate: f32, threshold_dbfs: f32, trigger_ms: f32) -> Self {
        let linear_rms = 10.0f32.powf(threshold_dbfs / 20.0);
        let frame_ms = 10.0f32;
        let trigger_frames = ((trigger_ms / frame_ms).round() as usize).max(3);

        Self {
            sample_rate: sample_rate.max(8000.0),
            energy_threshold: linear_rms,
            consecutive_howl_frames: 0,
            howl_trigger_frames: trigger_frames,
            prev_period: 0.0,
            triggered: false,
        }
    }

    /// Default configuration for 48 kHz 10 ms speech frames.
    pub fn default_48k() -> Self {
        Self::new(48000.0, -26.0, 60.0)
    }

    /// Resets detector state.
    pub fn reset(&mut self) {
        self.consecutive_howl_frames = 0;
        self.prev_period = 0.0;
        self.triggered = false;
    }

    /// Analyzes an audio frame for characteristics of runaway acoustic feedback:
    /// 1. High sustained RMS energy above threshold.
    /// 2. Low Crest Factor (sinusoidal waveform ~ 1.41 vs speech 3.0+).
    /// 3. Stable, stationary zero-crossing period (pure tonal oscillation).
    ///
    /// Returns `true` if acoustic feedback / howling is confirmed.
    pub fn detect(&mut self, frame: &[f32]) -> bool {
        if frame.len() < 32 {
            return false;
        }

        // 1. Compute RMS energy and peak amplitude
        let mut sum_sq = 0.0f32;
        let mut peak = 0.0f32;
        for &s in frame {
            let abs_s = s.abs();
            if abs_s > peak {
                peak = abs_s;
            }
            sum_sq += s * s;
        }

        let rms = (sum_sq / frame.len() as f32).sqrt();

        // If below energy threshold, feedback is physically impossible at this level
        if rms < self.energy_threshold || peak < 1e-4 {
            self.consecutive_howl_frames = self.consecutive_howl_frames.saturating_sub(1);
            return self.triggered;
        }

        // 2. Crest Factor: Peak / RMS
        // For a pure sine wave (howling tone), Crest Factor is sqrt(2) ≈ 1.414.
        // For normal human speech with transients and consonants, Crest Factor is typically 3.0 to 6.0+.
        let crest_factor = peak / rms.max(1e-6);

        // 3. Zero-Crossing Stability Analysis (Pitch Periodicity)
        let mut zero_crossings = 0usize;
        let mut first_crossing = None;
        let mut last_crossing = 0usize;

        for i in 1..frame.len() {
            if (frame[i - 1] <= 0.0 && frame[i] > 0.0) || (frame[i - 1] >= 0.0 && frame[i] < 0.0) {
                zero_crossings += 1;
                if first_crossing.is_none() {
                    first_crossing = Some(i);
                }
                last_crossing = i;
            }
        }

        let is_tonal = if zero_crossings >= 4 {
            if let Some(fc) = first_crossing {
                let avg_half_period = (last_crossing - fc) as f32 / (zero_crossings - 1) as f32;
                let est_freq = (self.sample_rate / (2.0 * avg_half_period.max(1.0))).clamp(100.0, 8000.0);

                // Howling typically occurs between 300 Hz and 5000 Hz in typical rooms
                let is_in_feedback_freq_range = (250.0..=5500.0).contains(&est_freq);
                let is_consistent_period = self.prev_period > 0.0 && (avg_half_period - self.prev_period).abs() < 1.5;

                self.prev_period = avg_half_period;
                is_in_feedback_freq_range && (is_consistent_period || zero_crossings >= 8)
            } else {
                false
            }
        } else {
            false
        };

        // Feedback signature: High energy + sinusoidal crest factor (< 2.2) + tonal stability
        if crest_factor < 2.2 && is_tonal {
            self.consecutive_howl_frames += 1;
            if self.consecutive_howl_frames >= self.howl_trigger_frames {
                self.triggered = true;
            }
        } else {
            self.consecutive_howl_frames = self.consecutive_howl_frames.saturating_sub(1);
        }

        self.triggered
    }

    /// Whether feedback has been triggered.
    #[inline]
    pub fn is_triggered(&self) -> bool {
        self.triggered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_howling_detection_on_pure_feedback_tone() {
        let mut detector = HowlingDetector::default_48k();

        // Generate 1000 Hz loud pure sine wave (simulating acoustic feedback howl at -10 dBFS)
        let sample_rate = 48000.0f32;
        let freq = 1000.0f32;
        let frame_size = 480;

        let mut triggered = false;
        // Feed 15 consecutive frames (150 ms)
        for frame_idx in 0..15 {
            let frame: Vec<f32> = (0..frame_size)
                .map(|i| {
                    let t = (frame_idx * frame_size + i) as f32 / sample_rate;
                    (2.0 * std::f32::consts::PI * freq * t).sin() * 0.35 // -9 dBFS
                })
                .collect();

            if detector.detect(&frame) {
                triggered = true;
                break;
            }
        }

        assert!(triggered, "Feedback detector failed to identify sustained 1000 Hz howl");
    }

    #[test]
    fn test_howling_rejection_on_natural_speech_simulation() {
        let mut detector = HowlingDetector::default_48k();
        let sample_rate = 48000.0f32;
        let frame_size = 480;

        // Simulate multi-tone speech harmonic with high crest factor (vowels + pauses)
        for frame_idx in 0..30 {
            let is_pause = frame_idx % 8 == 0;
            let frame: Vec<f32> = (0..frame_size)
                .map(|i| {
                    if is_pause {
                        0.001f32
                    } else {
                        let t = (frame_idx * frame_size + i) as f32 / sample_rate;
                        // Fundamental + 3 harmonics + pulse-like crest factor
                        let f0 = 150.0;
                        let s = (2.0 * std::f32::consts::PI * f0 * t).sin() * 0.2
                            + (2.0 * std::f32::consts::PI * (2.0 * f0) * t).sin() * 0.1
                            + (2.0 * std::f32::consts::PI * (3.0 * f0) * t).sin() * 0.05;
                        s * s * s.signum() // Accentuate peakiness (high crest factor)
                    }
                })
                .collect();

            let detected = detector.detect(&frame);
            assert!(!detected, "False positive: speech triggered howling detector!");
        }
    }

    #[test]
    fn test_howling_rejection_on_silence() {
        let mut detector = HowlingDetector::default_48k();
        let silence = vec![0.0001f32; 480];

        for _ in 0..20 {
            assert!(!detector.detect(&silence));
        }
    }
}
