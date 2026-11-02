/// Acoustic Echo Cancellation (AEC) module.
/// Cancels acoustic echo when the user listens through loudspeakers rather than headphones.
/// Utilizes a sample-accurate NLMS (Normalized Least Mean Squares) adaptive filter.
pub struct EchoCanceller {
    enabled: bool,
    filter_length: usize, // Typically 128 to 256 taps (~3 to 5 ms early reflections)
    weights: Vec<f32>,
    ref_ring: Vec<f32>,
    ring_idx: usize,
    step_size: f32,       // Adaptation learning rate mu
}

impl EchoCanceller {
    /// Creates a new EchoCanceller.
    ///
    /// # Arguments
    /// * `filter_length` - Number of adaptive FIR taps (e.g. 128 or 256).
    pub fn new(filter_length: usize) -> Self {
        let filter_length = filter_length.max(32);
        Self {
            enabled: false,
            filter_length,
            weights: vec![0.0f32; filter_length],
            ref_ring: vec![0.0f32; filter_length],
            ring_idx: 0,
            step_size: 0.1,
        }
    }

    pub fn default_preset() -> Self {
        Self::new(128)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn reset(&mut self) {
        self.weights.fill(0.0);
        self.ref_ring.fill(0.0);
        self.ring_idx = 0;
    }

    /// Feeds reference audio from the system/speaker render stream sample-by-sample.
    #[inline(always)]
    pub fn feed_reference_sample(&mut self, sample: f32) {
        self.ref_ring[self.ring_idx] = sample;
        self.ring_idx = (self.ring_idx + 1) % self.filter_length;
    }

    /// Feeds a chunk of reference audio samples.
    pub fn feed_reference(&mut self, ref_samples: &[f32]) {
        if !self.enabled {
            return;
        }
        for &s in ref_samples {
            self.feed_reference_sample(s);
        }
    }

    /// Cancels echo for a single sample given microphone signal and instantaneous reference.
    #[inline(always)]
    pub fn process_sample(&mut self, mic_sample: f32, ref_sample: f32) -> f32 {
        if !self.enabled {
            return mic_sample;
        }

        self.feed_reference_sample(ref_sample);

        let l = self.filter_length;
        let mut echo_hat = 0.0f32;
        let mut energy = 1e-6f32;

        // Compute predicted echo y_hat = sum_k (w[k] * x[n-k])
        for k in 0..l {
            let tap_idx = (self.ring_idx + l - 1 - k) % l;
            let x = self.ref_ring[tap_idx];
            echo_hat += self.weights[k] * x;
            energy += x * x;
        }

        let error = mic_sample - echo_hat;

        // NLMS weight update with near-end power constraint
        let norm_mu = self.step_size / energy;
        for k in 0..l {
            let tap_idx = (self.ring_idx + l - 1 - k) % l;
            let x = self.ref_ring[tap_idx];
            self.weights[k] += norm_mu * error * x;
        }

        error
    }

    /// Processes microphone frame and subtracts estimated loudspeaker echo in-place.
    pub fn process_mic_frame(&mut self, mic_frame: &mut [f32]) {
        if !self.enabled {
            return;
        }

        let l = self.filter_length;
        for sample in mic_frame.iter_mut() {
            let mic_val = *sample;
            let mut echo_hat = 0.0f32;
            let mut energy = 1e-6f32;

            for k in 0..l {
                let tap_idx = (self.ring_idx + l - 1 - k) % l;
                let x = self.ref_ring[tap_idx];
                echo_hat += self.weights[k] * x;
                energy += x * x;
            }

            let error = mic_val - echo_hat;

            // Only adapt when reference signal has significant power
            if energy > 1e-4 {
                let norm_mu = self.step_size / energy;
                for k in 0..l {
                    let tap_idx = (self.ring_idx + l - 1 - k) % l;
                    let x = self.ref_ring[tap_idx];
                    self.weights[k] += norm_mu * error * x;
                }
            }

            *sample = error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo_cancellation_convergence() {
        let mut aec = EchoCanceller::default_preset();
        aec.set_enabled(true);

        // Generate synthetic reference signal
        let ref_signal: Vec<f32> = (0..4800)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / 48000.0).sin() * 0.5)
            .collect();

        // Simulate echo with delay = 5 taps
        let mut mic_signal = vec![0.0f32; 4800];
        let delay = 5;
        for i in delay..4800 {
            mic_signal[i] = ref_signal[i - delay] * 0.4;
        }

        let mut output = vec![0.0f32; 4800];
        for i in 0..4800 {
            output[i] = aec.process_sample(mic_signal[i], ref_signal[i]);
        }

        // Verify echo is cancelled in the second half (> 12 dB attenuation)
        let initial_energy: f32 = output[100..600].iter().map(|&x| x * x).sum();
        let final_energy: f32 = output[3000..3500].iter().map(|&x| x * x).sum();

        assert!(
            final_energy < initial_energy * 0.25,
            "AEC did not attenuate echo: initial {}, final {}",
            initial_energy,
            final_energy
        );
    }
}
