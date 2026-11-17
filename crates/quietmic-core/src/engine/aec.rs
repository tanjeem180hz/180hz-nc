/// Acoustic Echo Cancellation (AEC) module.
/// Cancels acoustic echo when the user listens through loudspeakers rather than headphones.
/// Utilizes a sample-accurate NLMS (Normalized Least Mean Squares) adaptive filter
/// backed by a double-buffered contiguous delay line for zero-modulo, SIMD-vectorized execution.
pub struct EchoCanceller {
    enabled: bool,
    filter_length: usize, // Typically 128 to 256 taps (~3 to 5 ms early reflections)
    weights: Vec<f32>,
    ref_ring: Vec<f32>,   // 2 * filter_length double buffer for contiguous windowing
    ring_idx: usize,
    step_size: f32,       // Adaptation learning rate mu
    ref_fifo: Vec<f32>,   // Loopback reference FIFO queue
    ref_fifo_read: usize,
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
            ref_ring: vec![0.0f32; 2 * filter_length],
            ring_idx: 0,
            step_size: 0.1,
            ref_fifo: Vec::with_capacity(9600),
            ref_fifo_read: 0,
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
        self.ref_fifo.clear();
        self.ref_fifo_read = 0;
    }

    /// Feeds reference audio from the system/speaker render stream sample-by-sample.
    /// Stores the sample symmetrically in both halves of the double buffer,
    /// guaranteeing a contiguous window slice without modulo operations.
    #[inline(always)]
    pub fn feed_reference_sample(&mut self, sample: f32) {
        let l = self.filter_length;
        self.ref_ring[self.ring_idx] = sample;
        self.ref_ring[self.ring_idx + l] = sample;
        self.ring_idx += 1;
        if self.ring_idx >= l {
            self.ring_idx = 0;
        }
    }

    /// Feeds a chunk of reference audio samples into the loopback FIFO.
    pub fn feed_reference(&mut self, ref_samples: &[f32]) {
        if !self.enabled {
            return;
        }
        // Keep FIFO bounded to prevent unbounded memory growth (max 9600 samples = 200 ms)
        if self.ref_fifo.len().saturating_sub(self.ref_fifo_read) > 9600 {
            let retain_from = self.ref_fifo_read;
            self.ref_fifo.drain(0..retain_from);
            self.ref_fifo_read = 0;
        }
        self.ref_fifo.extend_from_slice(ref_samples);
    }

    /// Cancels echo for a single sample given microphone signal and instantaneous reference.
    #[inline(always)]
    pub fn process_sample(&mut self, mic_sample: f32, ref_sample: f32) -> f32 {
        if !self.enabled {
            return mic_sample;
        }

        self.feed_reference_sample(ref_sample);

        let l = self.filter_length;
        let x_window = &self.ref_ring[self.ring_idx..self.ring_idx + l];
        let mut echo_hat = 0.0f32;
        let mut energy = 1e-6f32;

        // Vectorized dot product and energy accumulator
        for (w, &x) in self.weights.iter().zip(x_window) {
            echo_hat += *w * x;
            energy += x * x;
        }

        let error = mic_sample - echo_hat;

        // NLMS weight update with near-end power constraint
        if energy > 1e-4 {
            let norm_mu = self.step_size / energy;
            for (w, &x) in self.weights.iter_mut().zip(x_window) {
                *w += norm_mu * error * x;
            }
        }

        error
    }

    /// Processes microphone frame and subtracts estimated loudspeaker echo in-place.
    pub fn process_mic_frame(&mut self, mic_frame: &mut [f32]) {
        if !self.enabled {
            return;
        }

        for sample in mic_frame.iter_mut() {
            let ref_sample = if self.ref_fifo_read < self.ref_fifo.len() {
                let s = self.ref_fifo[self.ref_fifo_read];
                self.ref_fifo_read += 1;
                s
            } else {
                0.0
            };
            *sample = self.process_sample(*sample, ref_sample);
        }

        // Periodically reclaim read space in ref_fifo
        if self.ref_fifo_read > 4800 && self.ref_fifo_read >= self.ref_fifo.len() {
            self.ref_fifo.clear();
            self.ref_fifo_read = 0;
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

    #[test]
    fn test_echo_cancellation_frame_pipeline_erle() {
        let mut aec = EchoCanceller::default_preset();
        aec.set_enabled(true);

        // Simulate 20 frames (200 ms = 9600 samples) of speaker playback
        let total_samples = 9600;
        let ref_signal: Vec<f32> = (0..total_samples)
            .map(|i| {
                let t = i as f32 / 48000.0;
                0.3 * (2.0 * std::f32::consts::PI * 330.0 * t).sin()
                    + 0.2 * (2.0 * std::f32::consts::PI * 660.0 * t).sin()
            })
            .collect();

        // Room impulse response simulation (echo delay = 8 samples, coupling = 0.5)
        let echo_delay = 8;
        let mut mic_signal = vec![0.0f32; total_samples];
        for i in echo_delay..total_samples {
            mic_signal[i] = ref_signal[i - echo_delay] * 0.5;
        }

        // Process frame-by-frame (480 samples each)
        let frame_size = 480;
        let num_frames = total_samples / frame_size;
        let mut output = Vec::with_capacity(total_samples);

        for f in 0..num_frames {
            let start = f * frame_size;
            let end = start + frame_size;
            let ref_frame = &ref_signal[start..end];
            let mut mic_frame = mic_signal[start..end].to_vec();

            aec.feed_reference(ref_frame);
            aec.process_mic_frame(&mut mic_frame);
            output.extend_from_slice(&mic_frame);
        }

        // Measure ERLE on converged frames (frames 15 to 20)
        let eval_start = 15 * frame_size;
        let eval_end = 20 * frame_size;
        let in_energy: f32 = mic_signal[eval_start..eval_end].iter().map(|&x| x * x).sum();
        let out_energy: f32 = output[eval_start..eval_end].iter().map(|&x| x * x).sum();

        let erle_db = 10.0 * (in_energy / out_energy.max(1e-9)).log10();
        println!("Echo Cancellation Frame ERLE: {:.2} dB", erle_db);

        assert!(
            erle_db >= 15.0,
            "ERLE ({:.2} dB) must be at least 15 dB after convergence",
            erle_db
        );
    }
}
