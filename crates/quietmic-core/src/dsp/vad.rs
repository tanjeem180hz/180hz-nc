/// Voice Activity & Silence Detector with Hangover and Energy Hysteresis.
/// Allows the processing pipeline to bypass heavy ML inference during sustained background silence,
/// while waking up instantaneously (< 0 ms) on speech onset with zero clipped syllables.
#[derive(Debug, Clone)]
pub struct SilenceDetector {
    energy_threshold_rms: f32,
    hangover_frames: usize,
    current_hangover: usize,
    is_active: bool,
    pre_trigger_buffer: Vec<f32>,
    auto_sensitivity: bool,
    noise_floor_rms: f32,
}

impl SilenceDetector {
    /// Creates a new SilenceDetector.
    ///
    /// # Arguments
    /// * `threshold_dbfs` - RMS energy threshold in dBFS below which audio is classified as silent (e.g. -55.0 dBFS).
    /// * `hangover_frames` - Number of 10 ms frames to keep the engine active after signal drops below threshold (e.g. 20 frames = 200 ms).
    /// * `frame_size` - Audio frame length (typically 480 samples at 48 kHz).
    pub fn new(threshold_dbfs: f32, hangover_frames: usize, frame_size: usize) -> Self {
        let linear_rms = 10.0f32.powf(threshold_dbfs / 20.0);
        Self {
            energy_threshold_rms: linear_rms,
            hangover_frames,
            current_hangover: hangover_frames, // Start active
            is_active: true,
            pre_trigger_buffer: vec![0.0f32; frame_size],
            auto_sensitivity: false,
            noise_floor_rms: linear_rms,
        }
    }

    /// Standard preset for 48 kHz 10 ms speech frames.
    /// Uses sensitive -58.0 dBFS threshold (preserves whispers and quiet fricatives)
    /// and 35 frames (350 ms) of hangover to eliminate syllable clipping.
    pub fn default_48k() -> Self {
        let mut d = Self::new(-58.0, 35, 480);
        d.auto_sensitivity = true;
        d
    }

    /// Sets manual sensitivity threshold in dBFS.
    pub fn set_threshold_dbfs(&mut self, threshold_dbfs: f32) {
        let clamped = threshold_dbfs.clamp(-80.0, 0.0);
        self.energy_threshold_rms = 10.0f32.powf(clamped / 20.0);
    }

    /// Toggles automatic sensitivity adjustment.
    pub fn set_auto_sensitivity(&mut self, enabled: bool) {
        self.auto_sensitivity = enabled;
    }

    /// Whether automatic sensitivity adjustment is enabled.
    pub fn is_auto_sensitivity(&self) -> bool {
        self.auto_sensitivity
    }

    /// Returns current sensitivity threshold in dBFS.
    pub fn threshold_dbfs(&self) -> f32 {
        if self.energy_threshold_rms > 1e-6 {
            20.0 * self.energy_threshold_rms.log10()
        } else {
            -100.0
        }
    }

    /// Computes RMS energy of a frame and updates activation state.
    /// Returns `true` if speech/activity is detected or within hangover window (engine should process).
    /// Returns `false` if confirmed pure silence (engine may bypass).
    pub fn update(&mut self, frame: &[f32]) -> bool {
        let sum_sq: f32 = frame.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / frame.len().max(1) as f32).sqrt();

        if self.auto_sensitivity {
            if rms < self.noise_floor_rms * 2.0 {
                self.noise_floor_rms = self.noise_floor_rms * 0.98 + rms * 0.02;
            } else {
                self.noise_floor_rms = self.noise_floor_rms * 0.999 + rms * 0.001;
            }
            self.energy_threshold_rms = (self.noise_floor_rms * 2.0).clamp(0.0005, 0.08);
        }

        if rms >= self.energy_threshold_rms {
            // Signal above threshold -> immediately activate and reset hangover
            self.current_hangover = self.hangover_frames;
            self.is_active = true;
        } else if self.current_hangover > 0 {
            // Within hangover window -> maintain active state to prevent chopping tail phonemes
            self.current_hangover -= 1;
            self.is_active = true;
        } else {
            // Hangover expired -> enter silence sleep mode
            self.is_active = false;
        }

        // Keep copy for pre-trigger safety
        if frame.len() == self.pre_trigger_buffer.len() {
            self.pre_trigger_buffer.copy_from_slice(frame);
        }

        self.is_active
    }

    /// Whether the detector considers the current state active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Reference to the pre-trigger buffer (previous frame).
    #[inline]
    pub fn pre_trigger(&self) -> &[f32] {
        &self.pre_trigger_buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence_detection_and_hangover() {
        let mut vad = SilenceDetector::new(-50.0, 5, 480);
        let silence_frame = vec![0.0001f32; 480]; // ~ -80 dBFS
        let speech_frame = vec![0.1f32; 480]; // ~ -20 dBFS

        // Initially active due to starting hangover
        assert!(vad.update(&speech_frame));

        // Feed silence: should stay active for 5 hangover frames
        for _ in 0..5 {
            assert!(vad.update(&silence_frame), "Should stay active in hangover window");
        }

        // 6th silent frame: hangover expired, should now return false (sleep mode)
        assert!(!vad.update(&silence_frame), "Should be asleep after hangover expires");

        // Now feed speech: should IMMEDIATELY wake up on the first frame!
        assert!(vad.update(&speech_frame), "Should wake up immediately on speech onset");
    }
}
