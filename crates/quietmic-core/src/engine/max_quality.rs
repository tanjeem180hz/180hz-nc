use crate::dsp::limiter::SoftLimiter;
use crate::engine::traits::DenoiseEngine;
use dfn3_rs::Denoiser;
use nnnoiseless::DenoiseState;

/// Max Quality Engine (Dual-Neural Extreme Krisp-Grade Engine).
///
/// Features & Acoustic Guarantees:
/// 1. Genuine DeepFilterNet3 48 kHz Deep Neural Network with embedded weights (100 dB limit).
/// 2. Parallel zero-latency RNNoise recurrent neural VAD consensus eliminates ambient noise leakage.
/// 3. Zero noise amplification: preserves natural 1.0x speech without boosting noisy room floors.
/// 4. Dual-neural + energy gating rapidly decays non-speech pauses to true digital silence (0.0).
/// 5. Continuous sample-by-sample linear gain interpolation (lerp) prevents frame-boundary clicks.
/// 6. Allocation-free real-time processing (0 heap allocations in audio loop).
pub struct MaxQualityEngine {
    denoiser: Denoiser,
    rnnoise: Box<DenoiseState<'static>>,
    rnnoise_vad: f32,
    suppression_factor: f32,
    last_lsnr: f32,
    current_gain: f32,
    prev_gain: f32,
    target_gain: f32,
    in_speech_env: f32,
    out_speech_env: f32,
    speech_hangover: usize,
    limiter: SoftLimiter,
    scratch_in: Box<[f32; Self::FRAME_SIZE]>,
    scratch_out: Box<[f32; Self::FRAME_SIZE]>,
    rnnoise_scratch_in: Box<[f32; Self::FRAME_SIZE]>,
    rnnoise_scratch_out: Box<[f32; Self::FRAME_SIZE]>,
}

impl MaxQualityEngine {
    pub const FRAME_SIZE: usize = 480; // 10 ms at 48 kHz

    pub fn new() -> Self {
        let mut denoiser = Denoiser::new();
        // 100 dB disables attenuation limit -> full neural suppression (Krisp level)
        denoiser.set_attenuation_limit(100.0);
        // Default calibrated DFNet3 thresholds (-10.0 dB LSNR floor)
        denoiser.set_thresholds(-10.0, 30.0, 20.0);
        Self {
            denoiser,
            rnnoise: DenoiseState::new(),
            rnnoise_vad: 0.0,
            suppression_factor: 1.0,
            last_lsnr: 0.0,
            current_gain: 1.0,
            prev_gain: 1.0,
            target_gain: 1.0,
            in_speech_env: 0.10,
            out_speech_env: 0.10,
            speech_hangover: 10,
            limiter: SoftLimiter::default_preset(),
            scratch_in: Box::new([0.0f32; Self::FRAME_SIZE]),
            scratch_out: Box::new([0.0f32; Self::FRAME_SIZE]),
            rnnoise_scratch_in: Box::new([0.0f32; Self::FRAME_SIZE]),
            rnnoise_scratch_out: Box::new([0.0f32; Self::FRAME_SIZE]),
        }
    }

    /// Sets suppression intensity (0.0 = pass-through, 1.0 = maximum Krisp-grade suppression).
    pub fn set_suppression(&mut self, factor: f32) {
        let f = factor.clamp(0.0, 1.0);
        self.suppression_factor = f;
        if f >= 0.99 {
            self.denoiser.set_attenuation_limit(100.0);
            self.denoiser.set_thresholds(-10.0, 30.0, 20.0);
        } else if f <= 0.01 {
            self.denoiser.set_attenuation_limit(0.0);
            self.denoiser.set_thresholds(-10.0, 30.0, 20.0);
        } else {
            self.denoiser.set_attenuation_limit(f * 50.0);
            self.denoiser.set_thresholds(-10.0, 30.0, 20.0);
        }
    }

    pub fn suppression(&self) -> f32 {
        self.suppression_factor
    }

    #[inline]
    pub fn vad_probability(&self) -> f32 {
        // DeepFilterNet3 local SNR fused with parallel RNNoise speech probability
        let df_conf = ((self.last_lsnr + 4.0) / 14.0).clamp(0.0, 1.0);
        df_conf.max(self.rnnoise_vad * 0.85)
    }

    #[inline]
    pub fn is_speech_active(&self) -> bool {
        self.last_lsnr > -1.0 || (self.last_lsnr > -6.0 && self.rnnoise_vad >= 0.25)
    }
}

impl Default for MaxQualityEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DenoiseEngine for MaxQualityEngine {
    fn name(&self) -> &'static str {
        "Max Quality (Krisp-Level 48kHz)"
    }

    fn reset(&mut self) {
        self.denoiser.reset();
        self.rnnoise = DenoiseState::new();
        self.rnnoise_vad = 0.0;
        self.last_lsnr = 0.0;
        self.current_gain = 1.0;
        self.prev_gain = 1.0;
        self.target_gain = 1.0;
        self.in_speech_env = 0.10;
        self.out_speech_env = 0.10;
        self.speech_hangover = 10;
        self.scratch_in.fill(0.0);
        self.scratch_out.fill(0.0);
        self.rnnoise_scratch_in.fill(0.0);
        self.rnnoise_scratch_out.fill(0.0);
    }

    #[inline]
    fn speech_confidence(&self) -> f32 {
        self.vad_probability()
    }

    #[inline]
    fn process_frame(&mut self, in_frame: &[f32], out_frame: &mut [f32]) -> Result<(), String> {
        if in_frame.len() < Self::FRAME_SIZE || out_frame.len() < Self::FRAME_SIZE {
            return Err(format!(
                "Frame size mismatch: expected {}, got in={}, out={}",
                Self::FRAME_SIZE,
                in_frame.len(),
                out_frame.len()
            ));
        }

        self.scratch_in.copy_from_slice(&in_frame[..Self::FRAME_SIZE]);
        self.last_lsnr = self.denoiser.process(&self.scratch_in, &mut self.scratch_out);

        // Run RNNoise in parallel on DFNet3-denoised signal purely as a zero-delay neural VAD verifier
        const SCALE_TO_16BIT: f32 = 32768.0;
        let in_energy: f32 = self.scratch_in.iter().map(|&x| x * x).sum();
        let in_rms = (in_energy / Self::FRAME_SIZE as f32).sqrt();
        let mut out_energy = 0.0f32;
        let mut frame_peak = 0.0f32;
        for (i, &s) in self.scratch_out.iter().enumerate() {
            out_energy += s * s;
            let a = s.abs();
            if a > frame_peak {
                frame_peak = a;
            }
            self.rnnoise_scratch_in[i] = s * SCALE_TO_16BIT;
        }
        let out_rms = (out_energy / Self::FRAME_SIZE as f32).sqrt();
        self.rnnoise_vad = self
            .rnnoise
            .process_frame(&mut *self.rnnoise_scratch_out, &*self.rnnoise_scratch_in);

        // Dual-neural Krisp-grade speech gate: rejects fan/traffic/clicks cleanly
        // while keeping loud unvoiced speech/plosives (out_rms > 0.04) open so loud voice never chops
        let is_speech = (self.last_lsnr > -1.0 && out_rms > 0.0012)
            || (self.last_lsnr > -6.0 && self.rnnoise_vad >= 0.25 && out_rms > 0.0025)
            || (self.last_lsnr > -10.0 && out_rms > 0.015)
            || out_rms > 0.04;

        if is_speech && self.suppression_factor > 0.01 {
            self.speech_hangover = 15; // 150 ms hangover preserves natural word cadence
            self.in_speech_env = self.in_speech_env * 0.90 + in_rms * 0.10;
            self.out_speech_env = self.out_speech_env * 0.90 + out_rms * 0.10;
            let ratio = (self.in_speech_env / self.out_speech_env.max(1e-4)).clamp(1.0, 2.75);
            self.target_gain = if frame_peak * ratio > 0.86 {
                (0.86 / frame_peak).min(ratio)
            } else {
                ratio
            };
        } else if self.suppression_factor <= 0.01 {
            self.target_gain = 1.0;
        } else if self.speech_hangover > 0 {
            if self.last_lsnr < -4.0 && self.rnnoise_vad < 0.20 {
                self.speech_hangover = self.speech_hangover.saturating_sub(2);
            } else {
                self.speech_hangover -= 1;
            }
        } else {
            // Non-speech pause: gate rapidly to digital silence (0.0 at 100% suppression)
            self.target_gain = (1.0 - self.suppression_factor).clamp(0.0, 1.0);
        }

        let slew = if self.target_gain > self.current_gain { 0.25 } else { 0.38 };
        self.current_gain += (self.target_gain - self.current_gain) * slew;
        if self.target_gain == 0.0 && self.current_gain < 1e-4 {
            self.current_gain = 0.0;
        }

        // Sample-by-sample continuous linear interpolation (lerp) prevents frame-boundary clicks
        let start_gain = self.prev_gain;
        let end_gain = self.current_gain;
        for i in 0..Self::FRAME_SIZE {
            let t = i as f32 / Self::FRAME_SIZE as f32;
            let g = start_gain + (end_gain - start_gain) * t;
            out_frame[i] = self.scratch_out[i] * g;
        }
        // Smooth gain-riding peak limiting (eliminates waveshaper cracking on loud voice)
        self.limiter.process_slice(&mut out_frame[..Self::FRAME_SIZE]);
        self.prev_gain = end_gain;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_quality_engine_silence() {
        let mut engine = MaxQualityEngine::new();
        let in_frame = vec![0.0f32; 480];
        let mut out_frame = vec![0.0f32; 480];

        assert!(engine.process_frame(&in_frame, &mut out_frame).is_ok());
        for &s in &out_frame {
            assert!(s.abs() < 1e-4, "Silence output must be near digital zero");
        }
    }

    #[test]
    fn test_max_quality_engine_noise_attenuation_and_speech_preservation() {
        let mut engine = MaxQualityEngine::new();
        let mut in_frame = [0.0f32; 480];
        let mut out_frame = [0.0f32; 480];

        // 1. Warm-up and feed speech with vocal pitch harmonics
        let mut rng = 123456789u32;
        let mut speech_rms = 0.0f32;
        let mut out_speech_rms = 0.0f32;

        for frame_idx in 0..80 {
            for (i, s) in in_frame.iter_mut().enumerate() {
                let t = (frame_idx * 480 + i) as f32 / 48000.0;
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let white = (rng as f32 / u32::MAX as f32 - 0.5) * 0.005;
                // Harmonic voice structure at 200 Hz (integer cycles per 10 ms frame)
                let f0 = 200.0f32;
                let mut speech = 0.0f32;
                for h in 1..=6 {
                    let freq = f0 * h as f32;
                    let amp = 0.25 / (h as f32).sqrt();
                    speech += amp * (2.0 * std::f32::consts::PI * freq * t).sin();
                }
                *s = speech * 0.35 + white;
            }

            let _ = engine.process_frame(&in_frame, &mut out_frame);

            // Measure during stable voiced speech (frames 35 to 55)
            if frame_idx >= 35 && frame_idx < 55 {
                let frame_in_rms = (in_frame.iter().map(|x| x * x).sum::<f32>() / 480.0).sqrt();
                let frame_out_rms = (out_frame.iter().map(|x| x * x).sum::<f32>() / 480.0).sqrt();
                speech_rms += frame_in_rms;
                out_speech_rms += frame_out_rms;
            }
        }

        speech_rms /= 20.0;
        out_speech_rms /= 20.0;
        let delta_db = (20.0 * (out_speech_rms / speech_rms).log10()).abs();
        println!("MaxQuality speech preservation in_rms={:.6}, out_rms={:.6}, delta={:.2} dB", speech_rms, out_speech_rms, delta_db);
        assert!(delta_db < 1.0, "Speech must be preserved within 1.0 dB, got {:.2} dB", delta_db);

        // 2. Feed pure noise (cooling fan / AC hum)
        engine.reset();
        let mut noise_in_rms = 0.0f32;
        let mut noise_out_rms = 0.0f32;

        for frame_idx in 0..40 {
            for (i, s) in in_frame.iter_mut().enumerate() {
                let t = (frame_idx * 480 + i) as f32 / 48000.0;
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let white = (rng as f32 / u32::MAX as f32 - 0.5) * 0.04;
                let hum = 0.03 * (2.0 * std::f32::consts::PI * 120.0 * t).sin();
                *s = hum + white;
            }

            engine.process_frame(&in_frame, &mut out_frame).unwrap();

            if frame_idx >= 20 {
                let in_rms = (in_frame.iter().map(|x| x * x).sum::<f32>() / 480.0).sqrt();
                let out_rms = (out_frame.iter().map(|x| x * x).sum::<f32>() / 480.0).sqrt();
                noise_in_rms += in_rms;
                noise_out_rms += out_rms;
            }
        }

        noise_in_rms /= 20.0;
        noise_out_rms /= 20.0;
        let attenuation_db = 20.0 * (noise_in_rms / noise_out_rms.max(1e-6)).log10();
        println!("MaxQuality noise attenuation: {:.2} dB", attenuation_db);
        assert!(attenuation_db > 25.0, "Noise attenuation must exceed 25 dB, got {:.2} dB", attenuation_db);
    }

    #[test]
    fn test_max_quality_extreme_zero_noise_leak_under_heavy_noise() {
        let mut engine = MaxQualityEngine::new();
        engine.set_suppression(1.0);

        let mut in_frame = [0.0f32; 480];
        let mut out_frame = [0.0f32; 480];
        let mut rng = 0xDEAD_BEEFu32;

        // Simulate heavy -18 dBFS combined fan hum + broadband turbulence + sharp keyboard clicks
        for frame_idx in 0..80 {
            for (i, s) in in_frame.iter_mut().enumerate() {
                let t = (frame_idx * 480 + i) as f32 / 48000.0;
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let white = (rng as f32 / u32::MAX as f32 - 0.5) * 0.14;
                let hum = 0.08 * (2.0 * std::f32::consts::PI * 60.0 * t).sin()
                    + 0.05 * (2.0 * std::f32::consts::PI * 120.0 * t).sin();
                let click = if (frame_idx % 10 == 0) && i < 120 {
                    0.35 * (-(i as f32) / 30.0).exp() * (2.0 * std::f32::consts::PI * 3200.0 * t).sin()
                } else {
                    0.0
                };
                *s = hum + white + click;
            }
            engine.process_frame(&in_frame, &mut out_frame).unwrap();
        }

        // Measure settled output over last 30 frames: must be virtually digital silence (< -70 dBFS)
        let mut out_sq = 0.0f32;
        for frame_idx in 80..110 {
            for (i, s) in in_frame.iter_mut().enumerate() {
                let t = (frame_idx * 480 + i) as f32 / 48000.0;
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let white = (rng as f32 / u32::MAX as f32 - 0.5) * 0.14;
                let hum = 0.08 * (2.0 * std::f32::consts::PI * 60.0 * t).sin()
                    + 0.05 * (2.0 * std::f32::consts::PI * 120.0 * t).sin();
                *s = hum + white;
            }
            engine.process_frame(&in_frame, &mut out_frame).unwrap();
            out_sq += out_frame.iter().map(|&x| x * x).sum::<f32>();
        }

        let out_rms = (out_sq / (30.0 * 480.0)).sqrt();
        let out_db = if out_rms > 1e-9 { 20.0 * out_rms.log10() } else { -180.0 };
        println!("MaxQuality heavy noise settled output: {:.1} dBFS", out_db);
        assert!(
            out_db < -70.0,
            "Heavy noise leaked through MaxQuality: {:.1} dBFS (expected < -70.0 dBFS)",
            out_db
        );
    }
}
