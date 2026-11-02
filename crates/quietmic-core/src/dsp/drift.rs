/// Adaptive Clock Drift Compensator.
/// Maintains constant latency and eliminates buffer overrun/underrun caused by independent crystal
/// clock drift between input capture hardware and output virtual audio hardware over long sessions (1h+).
#[derive(Debug, Clone)]
pub struct DriftCompensator {
    target_fill: f32,
    smoothed_fill: f32,
    smoothing_factor: f32,
    drift_threshold: f32,
    consecutive_adjustments: usize,
}

impl DriftCompensator {
    /// Creates a new DriftCompensator.
    ///
    /// # Arguments
    /// * `target_fill_samples` - Desired steady-state buffer fullness in samples (e.g. 960 = 2 frames of 480).
    /// * `tolerance_samples` - Allowable buffer deviation before drift compensation engages (e.g. 240 samples = 0.5 frame).
    pub fn new(target_fill_samples: usize, tolerance_samples: usize) -> Self {
        Self {
            target_fill: target_fill_samples as f32,
            smoothed_fill: target_fill_samples as f32,
            smoothing_factor: 0.01, // Slow exponential moving average
            drift_threshold: tolerance_samples as f32,
            consecutive_adjustments: 0,
        }
    }

    /// Default configuration for 48 kHz 10 ms frames with 2-frame target latency (20 ms).
    pub fn default_48k() -> Self {
        Self::new(960, 240)
    }

    /// Updates observed buffer fill count and returns the sample adjustment count (-1, 0, or +1).
    /// - `0`: No adjustment needed (clocks are aligned).
    /// - `-1`: Buffer is accumulating samples (input faster than output); skip 1 sample smoothly.
    /// - `+1`: Buffer is draining samples (input slower than output); insert 1 interpolated sample.
    pub fn update(&mut self, current_fill: usize) -> i32 {
        self.smoothed_fill += self.smoothing_factor * (current_fill as f32 - self.smoothed_fill);
        let error = self.smoothed_fill - self.target_fill;

        if error > self.drift_threshold {
            // Overfill: input running faster than output
            self.consecutive_adjustments += 1;
            // Only adjust at spaced intervals to prevent audio frequency modulation
            if self.consecutive_adjustments >= 10 {
                self.consecutive_adjustments = 0;
                // Nudge smoothed fill down slightly towards target
                self.smoothed_fill -= 1.0;
                return -1;
            }
        } else if error < -self.drift_threshold {
            // Underfill: input running slower than output
            self.consecutive_adjustments += 1;
            if self.consecutive_adjustments >= 10 {
                self.consecutive_adjustments = 0;
                self.smoothed_fill += 1.0;
                return 1;
            }
        } else {
            self.consecutive_adjustments = 0;
        }

        0
    }

    /// Adjusts an audio frame using high-quality linear interpolation when drift correction is triggered.
    /// Returns the new frame length (original length - 1, original length, or original length + 1).
    pub fn adjust_frame(&mut self, frame: &[f32], output: &mut Vec<f32>, current_fill: usize) {
        let adj = self.update(current_fill);
        output.clear();

        if adj == 0 || frame.len() < 2 {
            output.extend_from_slice(frame);
        } else if adj < 0 {
            // Drop 1 sample: blend middle two samples to prevent phase discontinuity
            let mid = frame.len() / 2;
            output.extend_from_slice(&frame[..mid]);
            // Crossfade middle transition
            let blended = 0.5 * (frame[mid] + frame[mid + 1]);
            output.push(blended);
            if mid + 2 < frame.len() {
                output.extend_from_slice(&frame[mid + 2..]);
            }
        } else {
            // Insert 1 sample: interpolate middle sample
            let mid = frame.len() / 2;
            output.extend_from_slice(&frame[..mid]);
            let inserted = 0.5 * (frame[mid - 1] + frame[mid]);
            output.push(inserted);
            output.extend_from_slice(&frame[mid..]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drift_correction_under_stress() {
        let mut compensator = DriftCompensator::new(100, 20);
        let frame = vec![0.5f32; 480];
        let mut out = Vec::with_capacity(500);

        // Simulate steady buffer
        for _ in 0..50 {
            compensator.adjust_frame(&frame, &mut out, 100);
            assert_eq!(out.len(), 480);
        }

        // Simulate high buffer overfill (e.g. 150 > target 100 + threshold 20)
        let mut dropped = false;
        for _ in 0..100 {
            compensator.adjust_frame(&frame, &mut out, 160);
            if out.len() == 479 {
                dropped = true;
                break;
            }
        }
        assert!(dropped, "Drift compensator failed to drop sample on buffer overfill");
    }
}
