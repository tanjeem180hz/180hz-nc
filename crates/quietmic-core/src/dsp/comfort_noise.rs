/// Comfort Noise Generator.
/// Generates an extremely subtle, low-pass shaped pseudo-random acoustic presence floor (~ -68 dBFS)
/// to avoid the unnatural "dead silence" digital cutoff perception during pauses or strong gating.
#[derive(Debug, Clone)]
pub struct ComfortNoiseGenerator {
    state: u32,
    lpf_state: f32,
    amplitude: f32,
    enabled: bool,
}

impl ComfortNoiseGenerator {
    /// Creates a new ComfortNoiseGenerator.
    ///
    /// # Arguments
    /// * `level_db` - Target level in dBFS (e.g. -68.0 dBFS)
    pub fn new(level_db: f32) -> Self {
        // Boost linear scale to account for the single-pole low pass filter gain
        let linear_amp = 10.0f32.powf(level_db / 20.0) * 3.5;
        Self {
            state: 0x180E_C0DE,
            lpf_state: 0.0,
            amplitude: linear_amp,
            enabled: true,
        }
    }

    /// Default generator (-68 dBFS floor).
    pub fn default_preset() -> Self {
        Self::new(-68.0)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Fast XorShift32 PRNG without std overhead or allocations.
    #[inline(always)]
    fn next_random_f32(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        // Convert to [-1.0, 1.0]
        ((x as f32) / (u32::MAX as f32)) * 2.0 - 1.0
    }

    /// Injects shaped comfort noise into the audio buffer in-place.
    #[inline]
    pub fn inject_into(&mut self, buffer: &mut [f32]) {
        if !self.enabled || self.amplitude <= 0.0 {
            return;
        }

        let alpha = 0.25f32; // Gentle low-pass filter to soften high hiss
        for sample in buffer.iter_mut() {
            let white = self.next_random_f32();
            self.lpf_state += alpha * (white - self.lpf_state);
            *sample += self.lpf_state * self.amplitude;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comfort_noise_level() {
        let mut gen = ComfortNoiseGenerator::default_preset();
        let mut buffer = vec![0.0f32; 4800];
        gen.inject_into(&mut buffer);

        // Verify RMS amplitude is close to -68 dBFS (~ 0.000398)
        let sum_sq: f32 = buffer.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / buffer.len() as f32).sqrt();
        let rms_db = 20.0 * rms.log10();

        assert!(
            rms_db < -55.0 && rms_db > -85.0,
            "Comfort noise RMS out of expected range: {} dBFS",
            rms_db
        );
    }
}
