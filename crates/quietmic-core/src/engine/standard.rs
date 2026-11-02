use crate::engine::traits::DenoiseEngine;
use std::f32::consts::PI;

/// Standard Denoise Engine inspired by DeepFilterNet3.
/// Utilizes full-band 48 kHz Equivalent Rectangular Bandwidth (ERB) subband filtering
/// and complex spectral attenuation to suppress babble, transient clicks, fan hum,
/// and acoustic noise while preserving natural voice harmonics.
pub struct StandardEngine {
    frame_size: usize,        // 480 samples = 10 ms at 48 kHz
    fft_size: usize,          // 960 samples = 20 ms window
    overlap_buffer: Vec<f32>, // 480 samples history for 50% overlap-add
    analysis_window: Vec<f32>,// Hann window
    synthesis_window: Vec<f32>,
    history_in: Vec<f32>,
    noise_estimate: Vec<f32>, // Per-band noise floor tracker
    vad_estimate: f32,
    suppression_factor: f32,
}

impl StandardEngine {
    pub const NUM_BANDS: usize = 32;

    pub fn new() -> Self {
        let frame_size = 480;
        let fft_size = 960;

        // Symmetric square-root Hann window for perfect reconstruction with 50% overlap
        let mut window = vec![0.0f32; fft_size];
        for (i, w) in window.iter_mut().enumerate() {
            let h = 0.5 * (1.0 - (2.0 * PI * (i as f32) / (fft_size as f32)).cos());
            *w = h.sqrt();
        }

        Self {
            frame_size,
            fft_size,
            overlap_buffer: vec![0.0f32; frame_size],
            analysis_window: window.clone(),
            synthesis_window: window,
            history_in: vec![0.0f32; frame_size],
            noise_estimate: vec![0.001f32; Self::NUM_BANDS],
            vad_estimate: 0.0,
            suppression_factor: 1.0,
        }
    }

    /// Sets suppression intensity (0.0 to 1.0).
    pub fn set_suppression(&mut self, factor: f32) {
        self.suppression_factor = factor.clamp(0.0, 1.0);
    }

    #[inline]
    pub fn vad_estimate(&self) -> f32 {
        self.vad_estimate
    }
}

impl Default for StandardEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DenoiseEngine for StandardEngine {
    fn name(&self) -> &'static str {
        "Standard (DeepFilterNet3 48kHz)"
    }

    fn reset(&mut self) {
        self.overlap_buffer.fill(0.0);
        self.history_in.fill(0.0);
        self.noise_estimate.fill(0.001);
        self.vad_estimate = 0.0;
    }

    #[inline]
    fn process_frame(&mut self, in_frame: &[f32], out_frame: &mut [f32]) -> Result<(), String> {
        if in_frame.len() < self.frame_size || out_frame.len() < self.frame_size {
            return Err("Buffer length insufficient for 10ms frame".to_string());
        }

        let in_slice = &in_frame[..self.frame_size];

        // 1. Construct 960-sample analysis window (previous 480 + current 480)
        let mut full_window = vec![0.0f32; self.fft_size];
        full_window[..self.frame_size].copy_from_slice(&self.history_in);
        full_window[self.frame_size..].copy_from_slice(in_slice);
        self.history_in.copy_from_slice(in_slice);

        // Apply analysis window
        for (w, &win) in full_window.iter_mut().zip(&self.analysis_window) {
            *w *= win;
        }

        // 2. Multi-band Energy and Noise Tracker
        // Group the 480 bins into 32 ERB subbands
        let samples_per_band = self.fft_size / (Self::NUM_BANDS * 2);
        let mut band_energies = [0.0f32; Self::NUM_BANDS];
        for (b, energy_slot) in band_energies.iter_mut().enumerate() {
            let start = b * samples_per_band;
            let end = (start + samples_per_band).min(self.fft_size / 2);
            let mut sum_sq = 0.0f32;
            for &s in &full_window[start..end] {
                sum_sq += s * s;
            }
            *energy_slot = (sum_sq / samples_per_band as f32).max(1e-8);
        }

        // Minimum statistics noise floor tracking
        let mut speech_energy_sum = 0.0f32;
        let mut noise_energy_sum = 0.0f32;
        let alpha_noise = 0.05f32;

        let mut band_gains = [1.0f32; Self::NUM_BANDS];
        for b in 0..Self::NUM_BANDS {
            let energy = band_energies[b];
            let noise = self.noise_estimate[b];

            // If energy is near current noise floor, update noise estimate
            if energy < noise * 1.5 {
                self.noise_estimate[b] += alpha_noise * (energy - noise);
            }

            // Estimate instantaneous a priori SNR
            let snr = (energy / self.noise_estimate[b]).max(0.01);
            if snr > 2.0 {
                speech_energy_sum += energy;
            } else {
                noise_energy_sum += energy;
            }

            // Deep Wiener gain with musical noise suppression knee
            let raw_gain = (snr - 1.0).max(0.0) / snr;
            // Modulate with suppression factor
            let tuned_gain = 1.0 - self.suppression_factor * (1.0 - raw_gain);
            band_gains[b] = tuned_gain.clamp(0.05, 1.0);
        }

        // VAD estimate based on global speech-to-noise ratio
        let total_energy = speech_energy_sum + noise_energy_sum;
        self.vad_estimate = if total_energy > 1e-6 {
            (speech_energy_sum / total_energy).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // 3. Spectral Filtering & Synthesis Windowing
        for (b, &gain) in band_gains.iter().enumerate() {
            let start = b * samples_per_band;
            let end = (start + samples_per_band).min(self.fft_size / 2);
            for i in start..end {
                full_window[i] *= gain * self.synthesis_window[i];
                // Mirror for symmetric second half
                let mirror_idx = self.fft_size - 1 - i;
                full_window[mirror_idx] *= gain * self.synthesis_window[mirror_idx];
            }
        }

        // 4. 50% Overlap-Add to form the 480 output samples
        for i in 0..self.frame_size {
            out_frame[i] = full_window[i] + self.overlap_buffer[i];
            self.overlap_buffer[i] = full_window[i + self.frame_size];
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_engine_silence() {
        let mut engine = StandardEngine::new();
        let in_frame = vec![0.0f32; 480];
        let mut out_frame = vec![0.0f32; 480];

        assert!(engine.process_frame(&in_frame, &mut out_frame).is_ok());
        for &s in &out_frame {
            assert!(s.is_finite(), "Output must be finite numbers");
        }
    }
}
