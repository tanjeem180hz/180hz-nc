/// Soft-knee Peak Limiter with transparent saturation curve.
/// Prevents output audio from clipping (exceeding [-1.0, 1.0]) while preserving speech dynamics
/// without harsh square-wave distortion.
#[derive(Debug, Clone)]
pub struct SoftLimiter {
    threshold: f32,
    knee_width: f32,
}

impl SoftLimiter {
    /// Creates a SoftLimiter with the specified ceiling threshold.
    ///
    /// # Arguments
    /// * `threshold` - Linear threshold above which soft saturation begins (typically 0.85 = -1.4 dBFS).
    pub fn new(threshold: f32) -> Self {
        let threshold = threshold.clamp(0.5, 0.98);
        Self {
            threshold,
            knee_width: 1.0 - threshold,
        }
    }

    /// Standard limiter configuration with -1.0 dBFS threshold (0.89 linear).
    pub fn default_preset() -> Self {
        Self::new(0.89)
    }

    /// Processes a single sample through the soft saturation curve.
    /// Zero allocations, SIMD branch friendly.
    #[inline(always)]
    pub fn limit_sample(&self, x: f32) -> f32 {
        let abs_x = x.abs();
        if abs_x <= self.threshold {
            x
        } else if abs_x >= 1.0 + self.knee_width {
            // Asymptotic ceiling at 1.0
            x.signum() * (1.0 - 1.0e-5)
        } else {
            // Cubic Hermite polynomial transition from threshold to 1.0
            let t = (abs_x - self.threshold) / (1.0 + self.knee_width - self.threshold);
            let compressed = self.threshold + self.knee_width * (3.0 * t * t - 2.0 * t * t * t);
            x.signum() * compressed.min(0.9999)
        }
    }

    /// Limits an audio slice in-place.
    #[inline]
    pub fn process_slice(&self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.limit_sample(*sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pass_through_low_amplitude() {
        let limiter = SoftLimiter::default_preset();
        let val = 0.5f32;
        assert_eq!(limiter.limit_sample(val), val);
        assert_eq!(limiter.limit_sample(-val), -val);
    }

    #[test]
    fn test_limit_large_spikes() {
        let limiter = SoftLimiter::default_preset();
        let extreme_vals = [1.2f32, 2.0f32, 10.0f32, -5.0f32];
        for &v in &extreme_vals {
            let limited = limiter.limit_sample(v);
            assert!(
                limited.abs() <= 1.0,
                "Sample exceeded ceiling: v = {}, limited = {}",
                v,
                limited
            );
        }
    }
}
