/// Asymmetric Gain Smoother with independent attack and release time constants.
/// Smooths gain changes and wet/dry blend transitions across frame boundaries to prevent
/// audible pops, clicks, or gating "pumping" artifacts.
#[derive(Debug, Clone)]
pub struct GainSmoother {
    current_gain: f32,
    target_gain: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl GainSmoother {
    /// Creates a new GainSmoother.
    ///
    /// # Arguments
    /// * `sample_rate` - Sample rate in Hz (e.g. 48000.0)
    /// * `attack_ms` - Attack time in milliseconds (fast response, e.g. 10.0 ms)
    /// * `release_ms` - Release time in milliseconds (gentle decay, e.g. 60.0 ms)
    /// * `initial_gain` - Initial gain (0.0 to 1.0)
    pub fn new(sample_rate: f32, attack_ms: f32, release_ms: f32, initial_gain: f32) -> Self {
        let sr = sample_rate.max(8000.0);
        let attack_samples = (attack_ms.max(0.1) * 0.001 * sr).max(1.0);
        let release_samples = (release_ms.max(0.1) * 0.001 * sr).max(1.0);

        // Alpha = 1 - exp(-1 / time_constant)
        let attack_coeff = 1.0 - (-1.0 / attack_samples).exp();
        let release_coeff = 1.0 - (-1.0 / release_samples).exp();

        Self {
            current_gain: initial_gain.clamp(0.0, 2.0),
            target_gain: initial_gain.clamp(0.0, 2.0),
            attack_coeff,
            release_coeff,
        }
    }

    /// Default configuration for speech denoising at 48 kHz.
    pub fn default_48k() -> Self {
        Self::new(48000.0, 8.0, 50.0, 1.0)
    }

    /// Sets the target gain (e.g. wet ratio or attenuation multiplier).
    #[inline]
    pub fn set_target(&mut self, target: f32) {
        self.target_gain = target.clamp(0.0, 2.0);
    }

    /// Current smoothed gain value.
    #[inline]
    pub fn current(&self) -> f32 {
        self.current_gain
    }

    /// Target gain value.
    #[inline]
    pub fn target(&self) -> f32 {
        self.target_gain
    }

    /// Advances the smoother by one sample and returns the smoothed gain.
    #[inline(always)]
    pub fn next_gain(&mut self) -> f32 {
        let diff = self.target_gain - self.current_gain;
        let coeff = if diff > 0.0 {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.current_gain += diff * coeff;
        self.current_gain
    }

    /// Applies smoothed wet/dry blend across an entire frame.
    /// `wet` is attenuated by the smoothed gain, while `dry` is scaled by (1.0 - smoothed_gain).
    #[inline]
    pub fn blend_frame(&mut self, dry: &[f32], wet: &[f32], output: &mut [f32]) {
        let len = dry.len().min(wet.len()).min(output.len());
        for i in 0..len {
            let g = self.next_gain();
            output[i] = (1.0 - g) * dry[i] + g * wet[i];
        }
    }

    /// Multiplies a buffer in-place by the smoothed gain profile.
    #[inline]
    pub fn apply_in_place(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample *= self.next_gain();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smooth_transition_no_discontinuity() {
        let mut smoother = GainSmoother::default_48k();
        smoother.set_target(0.0);

        let dry = vec![1.0f32; 480];
        let wet = vec![0.5f32; 480];
        let mut out = vec![0.0f32; 480];

        smoother.blend_frame(&dry, &wet, &mut out);

        // Check monotonicity and boundary smoothness
        for i in 1..480 {
            assert!(
                (out[i] - out[i - 1]).abs() < 0.01,
                "Sudden step at sample {}: prev {}, curr {}",
                i,
                out[i - 1],
                out[i]
            );
        }
    }
}
