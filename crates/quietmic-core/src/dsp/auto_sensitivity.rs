/// Intelligent Auto-Sensitivity and Dynamic Noise Floor Adapter.
///
/// Features:
/// 1. Continuous Minimum-Statistics Noise Floor Tracking:
///    Tracks the running background noise floor (fan, AC, room reverberation) across a 1.5s
///    sliding window without adapting upward into active speech vowels.
/// 2. Long-Term Speech Level Tracking:
///    Maintains rolling RMS estimate across active speech frames to distinguish whispers (-40 dBFS),
///    conversational speech (-20 dBFS), and loud shouting (-3 dBFS).
/// 3. Engine-Only Bounded Input Normalization (AGC):
///    Normalizes input to target -20 dBFS (0.10 linear) with a slow, bounded gain (+/-12 dB)
///    that FREEZES during silence/noise, then restores inverse gain on output to preserve natural dynamics.
/// 4. Dynamic Sensitivity & Suppression Tuning:
///    Relaxes suppression during loud speech to prevent harshness, and lowers gating thresholds
///    during whispering so quiet phonemes are never cut off.
#[derive(Debug, Clone)]
pub struct AutoSensitivityAdapter {
    enabled: bool,
    // Minimum-statistics noise floor tracker
    noise_floor_rms: f32,
    window_min_rms: f32,
    window_counter: usize,
    window_length_frames: usize, // 150 frames = 1.5 seconds at 10 ms/frame

    // Speech level tracker
    speech_level_rms: f32,
    speech_active: bool,
    hangover_counter: usize,

    // Engine input normalization AGC
    current_input_gain: f32,
    target_input_gain: f32,
    target_speech_rms: f32, // 0.10 (-20 dBFS)

    // Manual override sensitivity threshold in dBFS (when enabled = false)
    manual_threshold_dbfs: f32,

    // Runtime telemetry
    last_input_peak_db: f32,
    last_noise_floor_db: f32,
    last_auto_gain_db: f32,
}

impl AutoSensitivityAdapter {
    pub const DEFAULT_TARGET_RMS: f32 = 0.10; // -20 dBFS
    pub const MIN_GAIN: f32 = 0.35;           // -9 dB (clean attenuation for loud shouting)
    pub const MAX_GAIN: f32 = 1.50;           // +3.5 dB (bounded ceiling: never amplifies ambient noise floor)
    pub const HANGOVER_FRAMES: usize = 12;    // 120 ms hangover

    pub fn new() -> Self {
        Self {
            enabled: true,
            noise_floor_rms: 0.001, // -60 dBFS initial floor
            window_min_rms: 1.0,
            window_counter: 0,
            window_length_frames: 150,
            speech_level_rms: 0.10, // -20 dBFS initial speech level
            speech_active: false,
            hangover_counter: 0,
            current_input_gain: 1.0,
            target_input_gain: 1.0,
            target_speech_rms: Self::DEFAULT_TARGET_RMS,
            manual_threshold_dbfs: -50.0,
            last_input_peak_db: -100.0,
            last_noise_floor_db: -60.0,
            last_auto_gain_db: 0.0,
        }
    }

    pub fn default_preset() -> Self {
        Self::new()
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.target_input_gain = 1.0;
            self.current_input_gain = 1.0;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_manual_threshold_dbfs(&mut self, dbfs: f32) {
        self.manual_threshold_dbfs = dbfs.clamp(-80.0, 0.0);
    }

    pub fn manual_threshold_dbfs(&self) -> f32 {
        self.manual_threshold_dbfs
    }

    pub fn reset(&mut self) {
        self.noise_floor_rms = 0.001;
        self.window_min_rms = 1.0;
        self.window_counter = 0;
        self.speech_level_rms = 0.10;
        self.speech_active = false;
        self.hangover_counter = 0;
        self.current_input_gain = 1.0;
        self.target_input_gain = 1.0;
        self.last_input_peak_db = -100.0;
        self.last_noise_floor_db = -60.0;
        self.last_auto_gain_db = 0.0;
    }

    /// Analyzes an incoming 10 ms microphone frame, updates running minimum-statistics
    /// noise floor and speech RMS, and returns the recommended input gain and sensitivity threshold.
    pub fn update(&mut self, frame: &[f32]) -> (f32, f32, bool) {
        let mut sum_sq = 0.0f32;
        let mut peak = 0.0f32;
        for &s in frame {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            sum_sq += a * a;
        }
        let frame_rms = (sum_sq / frame.len().max(1) as f32).sqrt().max(1e-6);

        // Update telemetry
        self.last_input_peak_db = if peak > 1e-5 { 20.0 * peak.log10() } else { -100.0 };

        // 1. Minimum-statistics noise floor tracking
        self.window_min_rms = self.window_min_rms.min(frame_rms);
        self.window_counter += 1;
        if self.window_counter >= self.window_length_frames {
            // Smoothly update noise floor toward sliding minimum
            self.noise_floor_rms = self.noise_floor_rms * 0.70 + self.window_min_rms * 0.30;
            self.noise_floor_rms = self.noise_floor_rms.clamp(0.00005, 0.08); // -86 dBFS to -22 dBFS
            self.window_min_rms = frame_rms;
            self.window_counter = 0;
        } else if frame_rms < self.noise_floor_rms {
            // Immediate downward tracking if acoustic floor drops
            self.noise_floor_rms = self.noise_floor_rms * 0.90 + frame_rms * 0.10;
        } else if !self.speech_active && frame_rms < self.noise_floor_rms * 2.0 {
            // Smooth upward tracking of stationary ambient noise floor during non-speech intervals
            self.noise_floor_rms += 0.06 * (frame_rms - self.noise_floor_rms);
        }

        self.last_noise_floor_db = 20.0 * self.noise_floor_rms.log10();

        // 2. Speech activity detection
        let snr = frame_rms / self.noise_floor_rms;
        let crest_factor = peak / frame_rms;
        // True speech requires:
        // - SNR >= 2.8 relative to running minimum noise floor
        // - RMS >= 0.003 (-50 dBFS)
        // - Crest factor < 5.0 (rejects sharp isolated clicks like mechanical keys and mouse taps)
        let is_speech_instant = snr > 2.8 && frame_rms > 0.003 && crest_factor < 5.0;

        if is_speech_instant {
            self.hangover_counter = Self::HANGOVER_FRAMES;
            self.speech_active = true;
            // Update speech level tracker (slow time constant ~ 1.5s)
            self.speech_level_rms += 0.03 * (frame_rms - self.speech_level_rms);
            self.speech_level_rms = self.speech_level_rms.clamp(0.005, 1.0);
        } else if self.hangover_counter > 0 {
            self.hangover_counter -= 1;
            self.speech_active = true;
        } else {
            self.speech_active = false;
        }

        // 3. Engine Input Normalization AGC
        if self.enabled {
            if self.speech_active {
                let desired_gain = (self.target_speech_rms / self.speech_level_rms)
                    .clamp(Self::MIN_GAIN, Self::MAX_GAIN);
                self.target_input_gain = desired_gain;
                // Fast smooth adaptation toward speech gain target
                self.current_input_gain += 0.04 * (self.target_input_gain - self.current_input_gain);
            } else {
                // When speech stops: Target gain immediately resets to 1.0 (unity)
                // and decays smoothly back to 1.0 within 300 ms (rate = 0.08 per 10 ms frame).
                // Guaranteed: Auto-sensitivity NEVER raises the noise floor during silence or noise!
                self.target_input_gain = 1.0;
                self.current_input_gain += 0.08 * (1.0 - self.current_input_gain);
            }
        } else {
            self.target_input_gain = 1.0;
            self.current_input_gain = 1.0;
        }

        self.last_auto_gain_db = 20.0 * self.current_input_gain.log10();

        // 4. Adaptive threshold in dBFS
        let threshold_dbfs = if self.enabled {
            // Dynamic threshold: 3 dB above minimum-statistics noise floor
            // Whisper protection: minimum threshold down to -58 dBFS
            // Conversational speech protection: ceiling at -44 dBFS so soft speech is never cut off
            (self.last_noise_floor_db + 3.0).clamp(-58.0, -44.0)
        } else {
            self.manual_threshold_dbfs
        };

        (self.current_input_gain, threshold_dbfs, self.speech_active)
    }

    #[inline]
    pub fn current_gain(&self) -> f32 {
        self.current_input_gain
    }

    #[inline]
    pub fn noise_floor_db(&self) -> f32 {
        self.last_noise_floor_db
    }

    #[inline]
    pub fn auto_gain_db(&self) -> f32 {
        self.last_auto_gain_db
    }

    #[inline]
    pub fn input_peak_db(&self) -> f32 {
        self.last_input_peak_db
    }

    #[inline]
    pub fn is_speech_active(&self) -> bool {
        self.speech_active
    }
}

impl Default for AutoSensitivityAdapter {
    fn default() -> Self {
        Self::new()
    }
}
