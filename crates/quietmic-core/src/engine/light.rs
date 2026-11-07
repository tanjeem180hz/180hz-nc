use crate::engine::traits::DenoiseEngine;
use nnnoiseless::DenoiseState;

/// Light Engine powered by RNNoise (pure Rust implementation via `nnnoiseless`).
/// Delivers ultra-low CPU consumption (< 1-2% on low-end hardware) with zero external C dependencies.
pub struct LightEngine {
    state: Box<DenoiseState<'static>>,
    vad_probability: f32,
}

impl LightEngine {
    pub const FRAME_SIZE: usize = nnnoiseless::FRAME_SIZE; // 480 samples = 10 ms at 48 kHz

    pub fn new() -> Self {
        Self {
            state: DenoiseState::new(),
            vad_probability: 0.0,
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

        // RNNoise operates on 480-sample chunks
        self.vad_probability = self.state.process_frame(
            &mut out_frame[..Self::FRAME_SIZE],
            &in_frame[..Self::FRAME_SIZE],
        );

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
}
