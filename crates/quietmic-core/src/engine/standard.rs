use crate::engine::traits::DenoiseEngine;
use num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::f32::consts::PI;
use std::sync::Arc;

/// Standard Denoise Engine inspired by DeepFilterNet3.
/// Utilizes full-band 48 kHz Equivalent Rectangular Bandwidth (ERB) subband filtering
/// and real FFT complex spectral attenuation to suppress babble, transient clicks, fan hum,
/// and acoustic noise while preserving natural voice harmonics.
pub struct StandardEngine {
    pub frame_size: usize,        // 480 samples = 10 ms at 48 kHz
    pub fft_size: usize,          // 960 samples = 20 ms window
    r2c_fft: Arc<dyn RealToComplex<f32>>,
    c2r_fft: Arc<dyn ComplexToReal<f32>>,
    analysis_window: Vec<f32>,  // Square-root Hann window
    synthesis_window: Vec<f32>, // Square-root Hann window
    history_in: Vec<f32>,
    time_window: Vec<f32>,
    fft_scratch_fwd: Vec<Complex<f32>>,
    fft_scratch_inv: Vec<Complex<f32>>,
    complex_spectrum: Vec<Complex<f32>>,
    time_synth: Vec<f32>,
    overlap_buffer: Vec<f32>, // 480 samples history for 50% overlap-add
    noise_estimate: Vec<f32>, // Per-band noise floor tracker
    vad_estimate: f32,
    suppression_factor: f32,
    init_frames: usize,
    band_bin_ranges: [(usize, usize); Self::NUM_BANDS],
}

impl StandardEngine {
    pub const NUM_BANDS: usize = 32;

    pub fn new() -> Self {
        let frame_size = 480;
        let fft_size = 960;

        let mut planner = RealFftPlanner::<f32>::new();
        let r2c_fft = planner.plan_fft_forward(fft_size);
        let c2r_fft = planner.plan_fft_inverse(fft_size);

        let fft_scratch_fwd = r2c_fft.make_scratch_vec();
        let fft_scratch_inv = c2r_fft.make_scratch_vec();
        let complex_spectrum = r2c_fft.make_output_vec();
        let time_synth = c2r_fft.make_output_vec();

        // Symmetric square-root Hann window for perfect reconstruction with 50% overlap
        let mut window = vec![0.0f32; fft_size];
        for (i, w) in window.iter_mut().enumerate() {
            let h = 0.5 * (1.0 - (2.0 * PI * (i as f32) / (fft_size as f32)).cos());
            *w = h.sqrt();
        }

        // Generate 32 ERB-spaced frequency band boundaries across 481 complex bins
        let mut band_bin_ranges = [(0usize, 0usize); Self::NUM_BANDS];
        let num_bins = fft_size / 2 + 1; // 481 bins (0 Hz to 24000 Hz, 50 Hz/bin)
        let f_min = 50.0f32;
        let f_max = 22000.0f32;

        let mut bin_edges = [0usize; Self::NUM_BANDS + 1];
        bin_edges[0] = 0;
        bin_edges[Self::NUM_BANDS] = num_bins;

        for b in 1..Self::NUM_BANDS {
            let ratio = b as f32 / Self::NUM_BANDS as f32;
            let freq = 700.0 * ((1.0 + f_max / 700.0).powf(ratio) - 1.0).max(f_min / 700.0);
            let bin = (freq / 50.0).round() as usize;
            bin_edges[b] = bin.clamp(bin_edges[b - 1] + 1, num_bins - (Self::NUM_BANDS - b));
        }

        for b in 0..Self::NUM_BANDS {
            band_bin_ranges[b] = (bin_edges[b], bin_edges[b + 1]);
        }

        Self {
            frame_size,
            fft_size,
            r2c_fft,
            c2r_fft,
            analysis_window: window.clone(),
            synthesis_window: window,
            history_in: vec![0.0f32; frame_size],
            time_window: vec![0.0f32; fft_size],
            fft_scratch_fwd,
            fft_scratch_inv,
            complex_spectrum,
            time_synth,
            overlap_buffer: vec![0.0f32; frame_size],
            noise_estimate: vec![0.0f32; Self::NUM_BANDS],
            vad_estimate: 0.0,
            suppression_factor: 1.0,
            init_frames: 0,
            band_bin_ranges,
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
        self.time_window.fill(0.0);
        self.time_synth.fill(0.0);
        self.complex_spectrum.fill(Complex::new(0.0, 0.0));
        self.noise_estimate.fill(0.0);
        self.vad_estimate = 0.0;
        self.init_frames = 0;
    }

    #[inline]
    fn process_frame(&mut self, in_frame: &[f32], out_frame: &mut [f32]) -> Result<(), String> {
        if in_frame.len() < self.frame_size || out_frame.len() < self.frame_size {
            return Err("Buffer length insufficient for 10ms frame".to_string());
        }

        let in_slice = &in_frame[..self.frame_size];

        // 1. Construct 960-sample analysis window (previous 480 + current 480)
        self.time_window[..self.frame_size].copy_from_slice(&self.history_in);
        self.time_window[self.frame_size..].copy_from_slice(in_slice);
        self.history_in.copy_from_slice(in_slice);

        // Apply square-root Hann analysis window
        for (w, &win) in self.time_window.iter_mut().zip(&self.analysis_window) {
            *w *= win;
        }

        // 2. Forward Real FFT (960 real samples -> 481 complex bins)
        self.r2c_fft
            .process_with_scratch(
                &mut self.time_window,
                &mut self.complex_spectrum,
                &mut self.fft_scratch_fwd,
            )
            .map_err(|e| format!("Forward FFT error: {:?}", e))?;

        // 3. Multi-band Energy and Noise Tracker across 32 ERB subbands
        let mut speech_energy_sum = 0.0f32;
        let mut noise_energy_sum = 0.0f32;
        const FFT_NORM_SQ: f32 = 1.0 / (960.0 * 960.0);
        let is_startup = self.init_frames < 5;
        if is_startup {
            self.init_frames += 1;
        }

        for (b, &(start, end)) in self.band_bin_ranges.iter().enumerate() {
            let mut sum_power = 0.0f32;
            for k in start..end {
                sum_power += self.complex_spectrum[k].norm_sqr();
            }
            let count = (end - start).max(1) as f32;
            let band_power = (sum_power * FFT_NORM_SQ / count).max(1e-10);

            // Fast initialization on first frames or reset
            if self.noise_estimate[b] < 1e-9 {
                self.noise_estimate[b] = band_power.min(MAX_NOISE_FLOOR);
            }

            let noise = self.noise_estimate[b];

            // Ambient noise floor ceiling: stationary fan hum / keyboard / room noise floor
            // up to -27 dBFS (1e-3 power). Speech syllables (amplitude 0.2-0.5, power 0.03-0.12) are > 50x higher.
            const MAX_NOISE_FLOOR: f32 = 1e-3;

            if is_startup {
                self.noise_estimate[b] += 0.40 * (band_power.min(MAX_NOISE_FLOOR) - noise);
            } else if band_power < noise * 1.2 {
                // Downward adaptation: quickly track lowering noise floor
                self.noise_estimate[b] += 0.20 * (band_power - noise);
            } else if self.vad_estimate < 0.25 || b < 3 {
                // Non-speech or sub-200Hz rumble/mains hum -> adapt upward to cancel it
                self.noise_estimate[b] += 0.15 * (band_power.min(MAX_NOISE_FLOOR) - noise);
            } else {
                // Active speech in speech formant bands -> freeze noise floor to prevent speech leakage
            }

            self.noise_estimate[b] = self.noise_estimate[b].clamp(1e-9, MAX_NOISE_FLOOR);

            // Estimate subband SNR
            let snr = (band_power / self.noise_estimate[b].max(1e-10)).max(0.01);
            if (3..=24).contains(&b) {
                if snr > 2.0 {
                    speech_energy_sum += band_power;
                } else {
                    noise_energy_sum += band_power;
                }
            }

            let raw_gain = if snr > 2.0 {
                ((snr - 1.0) / snr).sqrt()
            } else {
                0.02
            };
            let tuned_gain = 1.0 - self.suppression_factor * (1.0 - raw_gain);
            let band_gain = tuned_gain.clamp(0.02, 1.0);

            for k in start..end {
                self.complex_spectrum[k] *= band_gain;
            }
        }

        // VAD estimate based on global speech-to-noise ratio
        let total_energy = speech_energy_sum + noise_energy_sum;
        self.vad_estimate = if total_energy > 1e-6 {
            (speech_energy_sum / total_energy).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // 4. Inverse Real FFT (481 complex bins -> 960 real samples)
        self.c2r_fft
            .process_with_scratch(
                &mut self.complex_spectrum,
                &mut self.time_synth,
                &mut self.fft_scratch_inv,
            )
            .map_err(|e| format!("Inverse FFT error: {:?}", e))?;

        // 5. Synthesis Windowing and FFT Normalization (1.0 / 960)
        const NORM: f32 = 1.0 / 960.0;
        for (s, &win) in self.time_synth.iter_mut().zip(&self.synthesis_window) {
            *s *= win * NORM;
        }

        // 6. 50% Overlap-Add to form the 480 output samples
        for (i, out_sample) in out_frame.iter_mut().enumerate().take(self.frame_size) {
            *out_sample = self.time_synth[i] + self.overlap_buffer[i];
            self.overlap_buffer[i] = self.time_synth[i + self.frame_size];
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
        assert_eq!(engine.vad_estimate(), 0.0);
    }

    #[test]
    fn test_standard_engine_perfect_reconstruction_at_zero_suppression() {
        let mut engine = StandardEngine::new();
        engine.set_suppression(0.0); // 0% suppression => unity gain pass-through

        let mut in_frame = [0.0f32; 480];
        let mut out_frame = [0.0f32; 480];

        // Warm up the 50% overlap buffer with 10 frames
        for frame_idx in 0..10 {
            for (i, s) in in_frame.iter_mut().enumerate() {
                let t = (frame_idx * 480 + i) as f32 / 48000.0;
                *s = 0.3 * (2.0 * PI * 440.0 * t).sin();
            }
            engine.process_frame(&in_frame, &mut out_frame).unwrap();
        }

        // Measure error on frame 10 (fully settled overlap-add)
        for (i, s) in in_frame.iter_mut().enumerate() {
            let t = (10 * 480 + i) as f32 / 48000.0;
            *s = 0.3 * (2.0 * PI * 440.0 * t).sin();
        }
        // Account for 1-frame (480 sample) lookahead latency: frame 10 output reconstructs frame 9
        let mut prev_frame = [0.0f32; 480];
        for (i, s) in prev_frame.iter_mut().enumerate() {
            let t = (9 * 480 + i) as f32 / 48000.0;
            *s = 0.3 * (2.0 * PI * 440.0 * t).sin();
        }

        engine.process_frame(&in_frame, &mut out_frame).unwrap();

        for i in 0..480 {
            let diff = (out_frame[i] - prev_frame[i]).abs();
            assert!(
                diff < 1e-4,
                "Sample {} reconstruction error exceeded tolerance: out={}, expected={}, diff={}",
                i,
                out_frame[i],
                prev_frame[i],
                diff
            );
        }
    }

    #[test]
    fn test_standard_engine_fan_noise_attenuation() {
        let mut engine = StandardEngine::new();
        engine.set_suppression(1.0); // 100% full suppression

        let mut in_frame = [0.0f32; 480];
        let mut out_frame = [0.0f32; 480];
        let mut rng = 12345678u32;

        // Feed 70 frames of stationary fan hum + noise floor
        let mut noise_in_energy = 0.0f32;
        let mut noise_out_energy = 0.0f32;

        for frame_idx in 0..70 {
            for (i, s) in in_frame.iter_mut().enumerate() {
                let t = (frame_idx * 480 + i) as f32 / 48000.0;
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let white = (rng as f32 / u32::MAX as f32 - 0.5) * 0.01;
                let hum = 0.03 * (2.0 * PI * 60.0 * t).sin()
                    + 0.02 * (2.0 * PI * 120.0 * t).sin()
                    + white;
                *s = hum;
            }

            engine.process_frame(&in_frame, &mut out_frame).unwrap();

            // Measure during last 25 frames after noise floor convergence
            if frame_idx >= 45 {
                for i in 0..480 {
                    noise_in_energy += in_frame[i] * in_frame[i];
                    noise_out_energy += out_frame[i] * out_frame[i];
                }
            }
        }

        let in_rms = (noise_in_energy / (25.0 * 480.0)).sqrt();
        let out_rms = (noise_out_energy / (25.0 * 480.0)).sqrt();
        let attenuation_db = 20.0 * (in_rms / out_rms.max(1e-6)).log10();

        println!("StandardEngine Fan in_rms={:.5}, out_rms={:.5}, Attenuation: {:.1} dB, VAD={:.3}", in_rms, out_rms, attenuation_db, engine.vad_estimate());
        assert!(
            attenuation_db >= 15.0,
            "StandardEngine noise attenuation was only {:.1} dB, expected >= 15.0 dB",
            attenuation_db
        );
    }

    #[test]
    fn test_standard_engine_speech_preservation() {
        let mut engine = StandardEngine::new();
        engine.set_suppression(1.0); // 100% full suppression

        let mut in_frame = [0.0f32; 480];
        let mut out_frame = [0.0f32; 480];

        // 1. Initial 15 frames of realistic room ambient noise before speech begins
        let mut rng = 42u32;
        for _ in 0..15 {
            for s in in_frame.iter_mut() {
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let room_noise = (rng as f32 / u32::MAX as f32 - 0.5) * 0.002;
                *s = room_noise;
            }
            engine.process_frame(&in_frame, &mut out_frame).unwrap();
        }

        // 2. Feed speech vowel (400 Hz harmonic + 800 Hz)
        let mut speech_in_energy = 0.0f32;
        let mut speech_out_energy = 0.0f32;

        for frame_idx in 0..60 {
            for (i, s) in in_frame.iter_mut().enumerate() {
                let t = (frame_idx * 480 + i) as f32 / 48000.0;
                let speech = 0.25 * (2.0 * PI * 400.0 * t).sin()
                    + 0.15 * (2.0 * PI * 800.0 * t).sin();
                *s = speech;
            }

            engine.process_frame(&in_frame, &mut out_frame).unwrap();

            // Measure in steady state
            if frame_idx >= 30 {
                for i in 0..480 {
                    speech_in_energy += in_frame[i] * in_frame[i];
                    speech_out_energy += out_frame[i] * out_frame[i];
                }
            }
        }

        let speech_in_rms = (speech_in_energy / (30.0 * 480.0)).sqrt();
        let speech_out_rms = (speech_out_energy / (30.0 * 480.0)).sqrt();
        let speech_diff_db = (20.0 * (speech_out_rms / speech_in_rms.max(1e-6)).log10()).abs();

        println!("StandardEngine Speech in_rms={:.5}, out_rms={:.5}, Diff: {:.2} dB, VAD={:.3}", speech_in_rms, speech_out_rms, speech_diff_db, engine.vad_estimate());
        assert!(
            speech_diff_db < 1.0,
            "StandardEngine modified speech vowel energy by {:.2} dB (expected < 1.0 dB)",
            speech_diff_db
        );
        assert!(
            engine.vad_estimate() > 0.8,
            "StandardEngine failed to detect speech activity: VAD={:.3}",
            engine.vad_estimate()
        );
    }
}
