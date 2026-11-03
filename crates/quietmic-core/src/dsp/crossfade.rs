use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrossfadeMode {
    Cleaned,
    Original,
}

/// Smooth, click-free A/B crossfader.
/// Interpolates exponentially between Cleaned (denoised) and Original (raw) audio
/// over a ~15 ms time window to eliminate pops, clicks, or phase artifacts during live comparison.
#[derive(Debug, Clone)]
pub struct CrossfadeSwitch {
    mode: CrossfadeMode,
    current_ratio: f32, // 0.0 = Cleaned, 1.0 = Original
    target_ratio: f32,
    alpha: f32,
}

impl CrossfadeSwitch {
    /// Creates a new CrossfadeSwitch.
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sample rate in Hz (e.g. 48000.0)
    /// * `crossfade_ms` - Transition duration in milliseconds to 95% completion (e.g. 15.0 ms)
    pub fn new(sample_rate: f32, crossfade_ms: f32) -> Self {
        let sr = sample_rate.max(8000.0);
        // Time constant tau = crossfade_ms / 3.0 so that 95% settling occurs within crossfade_ms
        let time_samples = (crossfade_ms.max(1.0) * 0.001 * sr / 3.0).max(1.0);
        let alpha = 1.0 - (-1.0 / time_samples).exp();

        Self {
            mode: CrossfadeMode::Cleaned,
            current_ratio: 0.0,
            target_ratio: 0.0,
            alpha,
        }
    }

    pub fn default_48k() -> Self {
        Self::new(48000.0, 15.0)
    }

    pub fn set_mode(&mut self, mode: CrossfadeMode) {
        self.mode = mode;
        self.target_ratio = match mode {
            CrossfadeMode::Cleaned => 0.0,
            CrossfadeMode::Original => 1.0,
        };
    }

    pub fn mode(&self) -> CrossfadeMode {
        self.mode
    }

    /// Current crossfade position (0.0 = pure cleaned, 1.0 = pure original).
    pub fn current_ratio(&self) -> f32 {
        self.current_ratio
    }

    /// Processes slices of raw and cleaned audio into output in-place.
    /// Guarantees continuous sample-by-sample linear interpolation without clicks.
    pub fn process_frame(&mut self, raw: &[f32], cleaned: &[f32], output: &mut [f32]) {
        let len = raw.len().min(cleaned.len()).min(output.len());

        for i in 0..len {
            self.current_ratio += (self.target_ratio - self.current_ratio) * self.alpha;
            let r = self.current_ratio;
            output[i] = (1.0 - r) * cleaned[i] + r * raw[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crossfade_click_free_transition() {
        let mut switch = CrossfadeSwitch::new(48000.0, 10.0); // 10 ms transition = 480 samples
        assert_eq!(switch.current_ratio(), 0.0);

        let raw = vec![1.0f32; 480];
        let cleaned = vec![0.0f32; 480];
        let mut out = vec![0.0f32; 480];

        // Initially in Cleaned mode -> output should be all 0.0
        switch.process_frame(&raw, &cleaned, &mut out);
        for &s in &out {
            assert_eq!(s, 0.0);
        }

        // Switch to Original mode -> should smoothly ramp up to ~1.0
        switch.set_mode(CrossfadeMode::Original);
        switch.process_frame(&raw, &cleaned, &mut out);

        // Verify strictly monotonic increase (no pops or clicks)
        for i in 1..out.len() {
            assert!(out[i] >= out[i - 1], "Crossfade had a negative step: {} -> {}", out[i - 1], out[i]);
            assert!(out[i].is_finite());
            assert!(out[i] <= 1.0);
        }
        assert!(out.last().unwrap() > &0.60, "Crossfade did not progress sufficiently towards 1.0");

        // Subsequent frame should settle near 1.0
        switch.process_frame(&raw, &cleaned, &mut out);
        assert!(out.last().unwrap() > &0.95);
    }
}
