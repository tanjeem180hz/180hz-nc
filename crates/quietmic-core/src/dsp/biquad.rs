use std::f32::consts::PI;

/// 2nd-order Butterworth High-Pass Filter implemented in Direct Form II Transposed.
/// Designed for real-time speech preprocessing (removing mic rumble, DC offset, and HVAC low-freq hum).
#[derive(Debug, Clone)]
pub struct HighPassFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    s1: f32,
    s2: f32,
}

impl HighPassFilter {
    /// Creates a new Butterworth 2nd-order high-pass filter.
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sampling rate in Hz (e.g. 48000.0)
    /// * `cutoff_hz` - Cutoff frequency (-3 dB point) in Hz (typically 80.0 Hz)
    pub fn new(sample_rate: f32, cutoff_hz: f32) -> Self {
        let sample_rate = sample_rate.max(8000.0);
        let cutoff_hz = cutoff_hz.clamp(10.0, sample_rate * 0.45);

        // Pre-warp frequency
        let omega = 2.0 * PI * cutoff_hz / sample_rate;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let q = 1.0 / std::f32::consts::SQRT_2; // Butterworth Q factor
        let alpha = sin_omega / (2.0 * q);

        let a0 = 1.0 + alpha;
        let b0 = ((1.0 + cos_omega) / 2.0) / a0;
        let b1 = (-(1.0 + cos_omega)) / a0;
        let b2 = ((1.0 + cos_omega) / 2.0) / a0;
        let a1 = (-2.0 * cos_omega) / a0;
        let a2 = (1.0 - alpha) / a0;

        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// Creates an 80 Hz cutoff filter at 48 kHz standard rate.
    pub fn default_48k() -> Self {
        Self::new(48000.0, 80.0)
    }

    /// Resets internal filter state history to 0.
    #[inline]
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    /// Processes a single audio sample in-place using Direct Form II Transposed.
    /// Free of allocations and branch mispredictions.
    #[inline(always)]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.s1;
        self.s1 = self.b1 * input - self.a1 * output + self.s2;
        self.s2 = self.b2 * input - self.a2 * output;
        output
    }

    /// Processes an audio slice in-place.
    #[inline]
    pub fn process_slice(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }
}

/// 2nd-order Parametric Peaking Filter implemented in Direct Form II Transposed.
/// Used for vocal presence enhancement (+2.0 dB at 3200 Hz) and speech clarity.
#[derive(Debug, Clone)]
pub struct PeakingFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    s1: f32,
    s2: f32,
}

impl PeakingFilter {
    /// Creates a new Peaking Filter.
    ///
    /// # Arguments
    /// * `sample_rate` - e.g. 48000.0
    /// * `center_freq_hz` - e.g. 3200.0
    /// * `gain_db` - e.g. +2.0
    /// * `q` - quality factor (bandwidth), e.g. 1.2
    pub fn new(sample_rate: f32, center_freq_hz: f32, gain_db: f32, q: f32) -> Self {
        let sample_rate = sample_rate.max(8000.0);
        let center_freq_hz = center_freq_hz.clamp(20.0, sample_rate * 0.45);
        let q = q.clamp(0.1, 10.0);

        let a = 10.0f32.powf(gain_db / 40.0);
        let omega = 2.0 * PI * center_freq_hz / sample_rate;
        let sin_omega = omega.sin();
        let cos_omega = omega.cos();
        let alpha = sin_omega / (2.0 * q);

        let b0_raw = 1.0 + alpha * a;
        let b1_raw = -2.0 * cos_omega;
        let b2_raw = 1.0 - alpha * a;
        let a0_raw = 1.0 + alpha / a;
        let a1_raw = -2.0 * cos_omega;
        let a2_raw = 1.0 - alpha / a;

        Self {
            b0: b0_raw / a0_raw,
            b1: b1_raw / a0_raw,
            b2: b2_raw / a0_raw,
            a1: a1_raw / a0_raw,
            a2: a2_raw / a0_raw,
            s1: 0.0,
            s2: 0.0,
        }
    }

    pub fn default_presence_48k() -> Self {
        Self::new(48000.0, 3200.0, 2.0, 1.2)
    }

    #[inline]
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    #[inline(always)]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.s1;
        self.s1 = self.b1 * input - self.a1 * output + self.s2;
        self.s2 = self.b2 * input - self.a2 * output;
        output
    }

    #[inline]
    pub fn process_slice(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dc_rejection() {
        let mut hpf = HighPassFilter::default_48k();
        // Feed constant DC signal of 1.0
        let mut signal = vec![1.0f32; 4800]; // 100 ms at 48 kHz
        hpf.process_slice(&mut signal);

        // After 100 ms, DC should be attenuated significantly (> 40 dB rejection)
        let last_samples = &signal[4000..];
        for &s in last_samples {
            assert!(s.abs() < 0.01, "DC signal not rejected: s = {}", s);
        }
    }

    #[test]
    fn test_passband_unity_gain() {
        let mut hpf = HighPassFilter::default_48k();
        let sample_rate = 48000.0f32;
        let freq = 1000.0f32; // 1 kHz sine wave well within passband

        let mut signal: Vec<f32> = (0..4800)
            .map(|n| (2.0 * PI * freq * (n as f32) / sample_rate).sin())
            .collect();

        hpf.process_slice(&mut signal);

        // Verify output amplitude in steady state (last 2000 samples) is approximately 1.0
        let max_val = signal[2800..].iter().copied().fold(0.0f32, f32::max);
        assert!((max_val - 1.0).abs() < 0.02, "Passband gain deviation: max = {}", max_val);
    }

    #[test]
    fn test_peaking_filter_gain() {
        let mut filter = PeakingFilter::new(48000.0, 3200.0, 6.0, 1.0); // +6 dB at 3200 Hz
        let sample_rate = 48000.0f32;
        let freq = 3200.0f32;

        let mut signal: Vec<f32> = (0..4800)
            .map(|n| 0.5 * (2.0 * PI * freq * (n as f32) / sample_rate).sin())
            .collect();

        filter.process_slice(&mut signal);

        // +6 dB means amplitude should double from 0.5 to ~1.0
        let max_val = signal[2800..].iter().copied().fold(0.0f32, f32::max);
        assert!((max_val - 1.0).abs() < 0.05, "Peaking boost expected ~1.0, got {}", max_val);
    }
}
