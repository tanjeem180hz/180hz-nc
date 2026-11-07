use crate::engine::traits::DenoiseEngine;
use nnnoiseless::DenoiseState;

/// Light Engine powered by RNNoise (pure Rust implementation via `nnnoiseless`).
/// Delivers ultra-low CPU consumption (< 1-2% on low-end hardware) with zero external C dependencies.
pub struct LightEngine {
    state: Box<DenoiseState<'static>>,
    vad_probability: f32,
    scratch_in: Box<[f32; Self::FRAME_SIZE]>,
    scratch_out: Box<[f32; Self::FRAME_SIZE]>,
}

impl LightEngine {
    pub const FRAME_SIZE: usize = nnnoiseless::FRAME_SIZE; // 480 samples = 10 ms at 48 kHz

    pub fn new() -> Self {
        Self {
            state: DenoiseState::new(),
            vad_probability: 0.0,
            scratch_in: Box::new([0.0; Self::FRAME_SIZE]),
            scratch_out: Box::new([0.0; Self::FRAME_SIZE]),
        }
    }

    /// Returns the Voice Activity probability estimated by RNNoise's recurrent model (0.0 to 1.0).
    #[inline]
    pub fn vad_probability(&self) -> f32 {
        self.vad_probability
    }
}

impl Default for LightEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DenoiseEngine for LightEngine {
    fn name(&self) -> &'static str {
        "Light (RNNoise 48kHz)"
    }

    fn reset(&mut self) {
        self.state = DenoiseState::new();
        self.vad_probability = 0.0;
        self.scratch_in.fill(0.0);
        self.scratch_out.fill(0.0);
    }

    #[inline]
    fn process_frame(&mut self, in_frame: &[f32], out_frame: &mut [f32]) -> Result<(), String> {
        if in_frame.len() < Self::FRAME_SIZE || out_frame.len() < Self::FRAME_SIZE {
            return Err(format!(
                "Frame size mismatch: expected at least {}, got in={}, out={}",
                Self::FRAME_SIZE,
                in_frame.len(),
                out_frame.len()
            ));
        }

        // RNNoise expects samples in 16-bit range [-32768.0, 32767.0]
        const SCALE_TO_16BIT: f32 = 32768.0;
        const SCALE_FROM_16BIT: f32 = 1.0 / 32768.0;

        for (i, &s) in in_frame[..Self::FRAME_SIZE].iter().enumerate() {
            self.scratch_in[i] = s * SCALE_TO_16BIT;
        }

        // RNNoise operates on 480-sample chunks
        self.vad_probability = self.state.process_frame(
            &mut *self.scratch_out,
            &*self.scratch_in,
        );

        for (i, &s) in self.scratch_out.iter().enumerate() {
            out_frame[i] = s * SCALE_FROM_16BIT;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nnnoiseless_scaling_behavior() {
        let mut st_a = DenoiseState::new();
        let mut out_a = [0.0f32; 480];
        let input_norm: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let mut vad_a = 0.0;
        for _ in 0..10 {
            vad_a = st_a.process_frame(&mut out_a, &input_norm);
        }
        let max_a = out_a.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        println!("Scenario A ([-1.0, 1.0]): vad={}, out max={}", vad_a, max_a);

        let mut st_b = DenoiseState::new();
        let mut out_b = [0.0f32; 480];
        let input_scaled: Vec<f32> = input_norm.iter().map(|&x| x * 32768.0).collect();
        let mut vad_b = 0.0;
        for _ in 0..10 {
            vad_b = st_b.process_frame(&mut out_b, &input_scaled);
        }
        let max_b = out_b.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        println!("Scenario B ([-32768, 32767]): vad={}, out max={}", vad_b, max_b);
    }

    #[test]
    fn test_light_engine_attenuation_and_speech_preservation() {
        let mut engine = LightEngine::new();
        let mut in_frame = [0.0f32; 480];
        let mut out_frame = [0.0f32; 480];

        // 1. Warm-up and test with speech mixed with noise
        let mut rng = 987654321u32;
        let mut vad_speech = 0.0f32;
        let mut vad_silence = 0.0f32;

        // Feed speech + noise for 50 frames
        for frame_idx in 0..50 {
            for (i, s) in in_frame.iter_mut().enumerate() {
                let t = (frame_idx * 480 + i) as f32 / 48000.0;
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let white = (rng as f32 / u32::MAX as f32 - 0.5) * 0.02;
                let speech = 0.20 * (2.0 * std::f32::consts::PI * 400.0 * t).sin()
                    + 0.15 * (2.0 * std::f32::consts::PI * 800.0 * t).sin();
                *s = speech + white;
            }

            engine.process_frame(&in_frame, &mut out_frame).unwrap();
            if frame_idx >= 30 {
                vad_speech = engine.vad_probability();
            }
        }

        // Feed noise only for 50 frames
        for frame_idx in 0..50 {
            for s in in_frame.iter_mut() {
                rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                let white = (rng as f32 / u32::MAX as f32 - 0.5) * 0.02;
                *s = white;
            }

            engine.process_frame(&in_frame, &mut out_frame).unwrap();
            if frame_idx >= 30 {
                vad_silence = engine.vad_probability();
            }
        }

        println!("LightEngine Speech VAD: {:.3}, Noise-only VAD: {:.3}", vad_speech, vad_silence);
        assert!(
            vad_speech > 0.8,
            "LightEngine failed to detect speech activity: VAD={:.3}",
            vad_speech
        );
        assert!(
            vad_silence < 0.2,
            "LightEngine failed to detect silence/noise activity: VAD={:.3}",
            vad_silence
        );

        // 2. Test speech vowel harmonic preservation
        let mut speech_in_energy = 0.0f32;
        let mut speech_out_energy = 0.0f32;

        for frame_idx in 0..50 {
            for (i, s) in in_frame.iter_mut().enumerate() {
                let t = (frame_idx * 480 + i) as f32 / 48000.0;
                let speech = 0.25 * (2.0 * std::f32::consts::PI * 400.0 * t).sin();
                *s = speech;
            }

            engine.process_frame(&in_frame, &mut out_frame).unwrap();

            if frame_idx >= 25 {
                for i in 0..480 {
                    speech_in_energy += in_frame[i] * in_frame[i];
                    speech_out_energy += out_frame[i] * out_frame[i];
                }
            }
        }

        let speech_in_rms = (speech_in_energy / (25.0 * 480.0)).sqrt();
        let speech_out_rms = (speech_out_energy / (25.0 * 480.0)).sqrt();
        let speech_diff_db = (20.0 * (speech_out_rms / speech_in_rms.max(1e-6)).log10()).abs();
        println!("LightEngine Speech Energy Preservation Diff: {:.2} dB", speech_diff_db);
        assert!(
            speech_diff_db < 1.0,
            "LightEngine modified speech vowel energy by {:.2} dB (expected < 1.0 dB)",
            speech_diff_db
        );
    }
}
