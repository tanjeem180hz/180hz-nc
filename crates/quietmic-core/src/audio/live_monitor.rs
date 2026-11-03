use crate::audio::device::DeviceManager;
use crate::dsp::crossfade::{CrossfadeMode, CrossfadeSwitch};
use crate::dsp::howling::HowlingDetector;
use crate::dsp::limiter::SoftLimiter;
use crate::dsp::resampler::AudioResampler;
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Status summary of the Live Test monitoring system reported to UI and IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveMonitorStatus {
    pub is_active: bool,
    pub ab_mode: String, // "Cleaned" or "Original"
    pub volume: f32,     // 0.0 to 1.0
    pub feedback_detected: bool,
    pub is_recording: bool,
    pub recording_seconds: f32,
    pub is_replaying: bool,
    pub replay_is_cleaned: bool,
    pub has_recorded_audio: bool,
    pub selected_device: Option<String>,
    pub error_message: Option<String>,
}

/// SPSC Ring buffer tap endpoints used to send raw and cleaned frames from the
/// main audio worker thread to the live monitor playback stream without locks or allocations.
pub struct LiveMonitorTapProducers {
    pub is_active: Arc<AtomicBool>,
    pub raw_prod: ringbuf::HeapProd<f32>,
    pub clean_prod: ringbuf::HeapProd<f32>,
}

impl LiveMonitorTapProducers {
    /// Pushes a processed 10 ms audio frame to the monitor stream if Live Test is active.
    /// Strict real-time guarantee: No allocations, no locks, no logging, no panics.
    #[inline(always)]
    pub fn push_frame(&mut self, raw: &[f32], clean: &[f32]) {
        if !self.is_active.load(Ordering::Relaxed) {
            return;
        }

        let len = raw.len().min(clean.len());
        for i in 0..len {
            let _ = self.raw_prod.try_push(raw[i]);
            let _ = self.clean_prod.try_push(clean[i]);
        }
    }
}

/// State of the 10-second Record & Replay tool.
pub struct RecordReplayState {
    pub raw_samples: Vec<f32>,
    pub clean_samples: Vec<f32>,
    pub max_samples: usize,
}

impl RecordReplayState {
    pub fn new(max_samples: usize) -> Self {
        Self {
            raw_samples: Vec::with_capacity(max_samples),
            clean_samples: Vec::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn clear(&mut self) {
        self.raw_samples.clear();
        self.clean_samples.clear();
    }
}

/// Real-time Live Test Monitor Controller.
/// Manages the monitor output stream to headphones, A/B crossfading,
/// volume scaling, ear-protection soft limiting, feedback howling detection,
/// and 10-second record-and-replay playback.
pub struct LiveMonitorController {
    is_active: Arc<AtomicBool>,
    ab_mode: Arc<AtomicU8>,       // 0 = Cleaned, 1 = Original
    volume_bits: Arc<AtomicU32>,   // f32::to_bits
    feedback_detected: Arc<AtomicBool>,
    is_recording: Arc<AtomicBool>,
    is_replaying: Arc<AtomicBool>,
    replay_is_cleaned: Arc<AtomicBool>,
    replay_cursor: Arc<AtomicUsize>,
    recording_count: Arc<AtomicUsize>,
    last_activity: Arc<parking_lot::Mutex<Instant>>,
    record_state: Arc<parking_lot::Mutex<RecordReplayState>>,
    selected_device_name: Arc<parking_lot::Mutex<Option<String>>>,
    last_error: Arc<parking_lot::Mutex<Option<String>>>,
    stream: Option<cpal::Stream>,
    raw_cons: Arc<parking_lot::Mutex<ringbuf::HeapCons<f32>>>,
    clean_cons: Arc<parking_lot::Mutex<ringbuf::HeapCons<f32>>>,
}

// Thread-safe handles
unsafe impl Send for LiveMonitorController {}
unsafe impl Sync for LiveMonitorController {}

impl LiveMonitorController {
    pub const BUFFER_CAPACITY: usize = 9600; // ~200 ms headroom at 48 kHz
    pub const MAX_RECORD_SECONDS: usize = 10;
    pub const SAMPLE_RATE_48K: u32 = 48000;
    pub const MAX_RECORD_SAMPLES: usize = Self::SAMPLE_RATE_48K as usize * Self::MAX_RECORD_SECONDS; // 480,000 samples

    pub fn new() -> (Self, LiveMonitorTapProducers) {
        let is_active = Arc::new(AtomicBool::new(false));
        let ab_mode = Arc::new(AtomicU8::new(0)); // Cleaned by default
        let volume_bits = Arc::new(AtomicU32::new(0.50f32.to_bits())); // 50% volume default
        let feedback_detected = Arc::new(AtomicBool::new(false));
        let is_recording = Arc::new(AtomicBool::new(false));
        let is_replaying = Arc::new(AtomicBool::new(false));
        let replay_is_cleaned = Arc::new(AtomicBool::new(false));
        let replay_cursor = Arc::new(AtomicUsize::new(0));
        let recording_count = Arc::new(AtomicUsize::new(0));
        let last_activity = Arc::new(parking_lot::Mutex::new(Instant::now()));
        let record_state = Arc::new(parking_lot::Mutex::new(RecordReplayState::new(Self::MAX_RECORD_SAMPLES)));
        let selected_device_name = Arc::new(parking_lot::Mutex::new(None));
        let last_error = Arc::new(parking_lot::Mutex::new(None));

        let raw_rb = HeapRb::<f32>::new(Self::BUFFER_CAPACITY);
        let clean_rb = HeapRb::<f32>::new(Self::BUFFER_CAPACITY);
        let (raw_prod, raw_cons) = raw_rb.split();
        let (clean_prod, clean_cons) = clean_rb.split();

        let tap_producers = LiveMonitorTapProducers {
            is_active: Arc::clone(&is_active),
            raw_prod,
            clean_prod,
        };

        let controller = Self {
            is_active,
            ab_mode,
            volume_bits,
            feedback_detected,
            is_recording,
            is_replaying,
            replay_is_cleaned,
            replay_cursor,
            recording_count,
            last_activity,
            record_state,
            selected_device_name,
            last_error,
            stream: None,
            raw_cons: Arc::new(parking_lot::Mutex::new(raw_cons)),
            clean_cons: Arc::new(parking_lot::Mutex::new(clean_cons)),
        };

        (controller, tap_producers)
    }

    /// Creates a fresh pair of tap producers connected to the controller's consumers.
    /// Used when restarting AudioStreamController.
    pub fn create_new_tap(&mut self) -> LiveMonitorTapProducers {
        let raw_rb = HeapRb::<f32>::new(Self::BUFFER_CAPACITY);
        let clean_rb = HeapRb::<f32>::new(Self::BUFFER_CAPACITY);
        let (raw_prod, raw_cons) = raw_rb.split();
        let (clean_prod, clean_cons) = clean_rb.split();

        *self.raw_cons.lock() = raw_cons;
        *self.clean_cons.lock() = clean_cons;

        LiveMonitorTapProducers {
            is_active: Arc::clone(&self.is_active),
            raw_prod,
            clean_prod,
        }
    }

    /// Starts live monitoring through the chosen physical output device (headphones/speakers).
    /// Strictly verifies that VB-CABLE is NOT selected.
    pub fn start(
        &mut self,
        device_manager: &DeviceManager,
        output_device_name: Option<&str>,
    ) -> Result<(), String> {
        self.stop();

        // 1. Resolve output device
        let dev = device_manager.get_output_device(output_device_name)?;
        let name = dev.name().unwrap_or_else(|_| "Default Audio Output".to_string());

        // Safety verification: Exclude VB-CABLE from ever receiving monitor audio
        let lower = name.to_lowercase();
        if lower.contains("cable input") || lower.contains("cable output") || lower.contains("vb-audio") {
            return Err("Cannot use VB-CABLE as live test monitor. Please select your headphones or speakers.".to_string());
        }

        tracing::info!("Starting Live Test monitor stream to: {}", name);
        *self.selected_device_name.lock() = Some(name.clone());
        *self.last_error.lock() = None;
        self.feedback_detected.store(false, Ordering::Relaxed);
        *self.last_activity.lock() = Instant::now();

        // Drain any stale samples in buffers
        if let Some(mut r) = self.raw_cons.try_lock() {
            while r.try_pop().is_some() {}
        }
        if let Some(mut c) = self.clean_cons.try_lock() {
            while c.try_pop().is_some() {}
        }

        // 2. Query output configuration
        let config = dev.default_output_config()
            .map_err(|e| format!("Failed to read monitor device configuration: {}", e))?;
        let target_sample_rate = config.sample_rate().0;
        let channels = (config.channels() as usize).max(1);

        // 3. Setup DSP helpers
        let mut crossfade = CrossfadeSwitch::new(Self::SAMPLE_RATE_48K as f32, 15.0);
        let mut howling_detector = HowlingDetector::default_48k();
        let limiter = SoftLimiter::default_preset();

        // Resampler if headphones operate at non-48kHz (e.g. 44.1 kHz, 96 kHz)
        let mut resampler = if target_sample_rate != Self::SAMPLE_RATE_48K {
            Some(AudioResampler::new(Self::SAMPLE_RATE_48K, target_sample_rate, 480)?)
        } else {
            None
        };

        // Shared atomics and queues for callback
        let is_active_cb = Arc::clone(&self.is_active);
        let ab_mode_cb = Arc::clone(&self.ab_mode);
        let volume_bits_cb = Arc::clone(&self.volume_bits);
        let feedback_cb = Arc::clone(&self.feedback_detected);
        let is_recording_cb = Arc::clone(&self.is_recording);
        let is_replaying_cb = Arc::clone(&self.is_replaying);
        let replay_is_cleaned_cb = Arc::clone(&self.replay_is_cleaned);
        let replay_cursor_cb = Arc::clone(&self.replay_cursor);
        let record_state_cb = Arc::clone(&self.record_state);
        let recording_count_cb = Arc::clone(&self.recording_count);
        let last_error_cb = Arc::clone(&self.last_error);
        let raw_cons_cb = Arc::clone(&self.raw_cons);
        let clean_cons_cb = Arc::clone(&self.clean_cons);

        let mut scratch_raw = vec![0.0f32; 480];
        let mut scratch_clean = vec![0.0f32; 480];
        let mut scratch_blended = vec![0.0f32; 480];
        let mut scratch_resampled = Vec::with_capacity(1000);

        // 4. Build Output Stream
        let err_cb_active = Arc::clone(&self.is_active);
        let err_cb_error = Arc::clone(&self.last_error);

        let stream = dev.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if !is_active_cb.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }

                // If feedback was detected, instantly mute output for safety
                if feedback_cb.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    is_active_cb.store(false, Ordering::Relaxed);
                    return;
                }

                let volume = f32::from_bits(volume_bits_cb.load(Ordering::Relaxed));

                // A. Replay Mode Audio
                if is_replaying_cb.load(Ordering::Relaxed) {
                    if let Some(rec) = record_state_cb.try_lock() {
                        let raw_len = rec.raw_samples.len();
                        let clean_len = rec.clean_samples.len();
                        let total_len = raw_len + clean_len;
                        let mut cursor = replay_cursor_cb.load(Ordering::Relaxed);

                        if total_len == 0 || cursor >= total_len {
                            is_replaying_cb.store(false, Ordering::Relaxed);
                            replay_cursor_cb.store(0, Ordering::Relaxed);
                            data.fill(0.0);
                            return;
                        }

                        for frame in data.chunks_exact_mut(channels) {
                            if cursor < raw_len {
                                replay_is_cleaned_cb.store(false, Ordering::Relaxed);
                                let s = rec.raw_samples[cursor] * volume;
                                for ch in frame.iter_mut() {
                                    *ch = s;
                                }
                                cursor += 1;
                            } else if cursor < total_len {
                                replay_is_cleaned_cb.store(true, Ordering::Relaxed);
                                let s = rec.clean_samples[cursor - raw_len] * volume;
                                for ch in frame.iter_mut() {
                                    *ch = s;
                                }
                                cursor += 1;
                            } else {
                                for ch in frame.iter_mut() {
                                    *ch = 0.0;
                                }
                            }
                        }
                        replay_cursor_cb.store(cursor, Ordering::Relaxed);
                        if cursor >= total_len {
                            is_replaying_cb.store(false, Ordering::Relaxed);
                            replay_cursor_cb.store(0, Ordering::Relaxed);
                        }
                    } else {
                        data.fill(0.0);
                    }
                    return;
                }

                // B. Live Capture Monitor Audio (10 ms chunks)
                let num_samples_needed = data.len() / channels;

                // Check A/B mode setting
                let target_ab = if ab_mode_cb.load(Ordering::Relaxed) == 1 {
                    CrossfadeMode::Original
                } else {
                    CrossfadeMode::Cleaned
                };
                crossfade.set_mode(target_ab);

                // Drain from raw and cleaned SPSC queues
                let mut samples_read = 0usize;
                if let (Some(mut r_lock), Some(mut c_lock)) = (raw_cons_cb.try_lock(), clean_cons_cb.try_lock()) {
                    while samples_read < 480 && samples_read < num_samples_needed {
                        let r = r_lock.try_pop().unwrap_or(0.0);
                        let c = c_lock.try_pop().unwrap_or(0.0);
                        scratch_raw[samples_read] = r;
                        scratch_clean[samples_read] = c;
                        samples_read += 1;
                    }
                }

                if samples_read == 0 {
                    data.fill(0.0);
                    return;
                }

                // Record frames if recording is active
                if is_recording_cb.load(Ordering::Relaxed) {
                    let current_count = recording_count_cb.load(Ordering::Relaxed);
                    if current_count < Self::MAX_RECORD_SAMPLES {
                        if let Some(mut rec) = record_state_cb.try_lock() {
                            rec.raw_samples.extend_from_slice(&scratch_raw[..samples_read]);
                            rec.clean_samples.extend_from_slice(&scratch_clean[..samples_read]);
                            let new_total = rec.raw_samples.len();
                            recording_count_cb.store(new_total, Ordering::Relaxed);
                            if new_total >= Self::MAX_RECORD_SAMPLES {
                                is_recording_cb.store(false, Ordering::Relaxed);
                            }
                        }
                    } else {
                        is_recording_cb.store(false, Ordering::Relaxed);
                    }
                }

                // Perform smooth click-free A/B crossfade
                crossfade.process_frame(
                    &scratch_raw[..samples_read],
                    &scratch_clean[..samples_read],
                    &mut scratch_blended[..samples_read],
                );

                // Run Howling / Acoustic Feedback Detection
                if howling_detector.detect(&scratch_blended[..samples_read]) {
                    feedback_cb.store(true, Ordering::Relaxed);
                    *last_error_cb.lock() = Some(
                        "Acoustic feedback (howling) detected! Monitor was automatically muted to protect your hearing. Please use headphones.".to_string()
                    );
                    data.fill(0.0);
                    is_active_cb.store(false, Ordering::Relaxed);
                    return;
                }

                // Apply volume scaling and ear-protection soft limiter
                for s in scratch_blended[..samples_read].iter_mut() {
                    *s *= volume;
                }
                limiter.process_slice(&mut scratch_blended[..samples_read]);

                // Resample to device native rate if needed
                let out_slice: &[f32] = if let Some(ref mut rs) = resampler {
                    scratch_resampled.clear();
                    if rs.process(&scratch_blended[..samples_read], &mut scratch_resampled).is_ok() {
                        &scratch_resampled
                    } else {
                        &scratch_blended[..samples_read]
                    }
                } else {
                    &scratch_blended[..samples_read]
                };

                // Interleave into output buffer
                for (out_idx, frame) in data.chunks_exact_mut(channels).enumerate() {
                    let sample = if out_idx < out_slice.len() {
                        out_slice[out_idx]
                    } else {
                        0.0
                    };
                    for ch in frame.iter_mut() {
                        *ch = sample;
                    }
                }
            },
            move |err| {
                tracing::warn!("Monitor playback stream error (device may have disconnected): {}", err);
                err_cb_active.store(false, Ordering::Relaxed);
                *err_cb_error.lock() = Some(format!("Monitor device error: {}", err));
            },
            None,
        ).map_err(|e| format!("Failed to create monitor playback stream: {}", e))?;

        stream.play().map_err(|e| format!("Failed to start monitor stream: {}", e))?;
        self.stream = Some(stream);
        self.is_active.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// Stops live monitoring and drops the audio stream immediately.
    pub fn stop(&mut self) {
        self.is_active.store(false, Ordering::Relaxed);
        self.is_recording.store(false, Ordering::Relaxed);
        self.is_replaying.store(false, Ordering::Relaxed);
        if let Some(st) = self.stream.take() {
            let _ = st.pause();
            drop(st);
        }
        tracing::info!("Live Test monitor stopped and stream dropped");
    }

    /// Toggles A/B crossfade between Cleaned (denoised) and Original (raw).
    pub fn set_ab_mode(&self, mode: CrossfadeMode) {
        let val = match mode {
            CrossfadeMode::Cleaned => 0,
            CrossfadeMode::Original => 1,
        };
        self.ab_mode.store(val, Ordering::Relaxed);
        *self.last_activity.lock() = Instant::now();
    }

    /// Sets monitor volume (0.0 to 1.0, clamped).
    pub fn set_volume(&self, volume: f32) {
        let v = volume.clamp(0.0, 1.0);
        self.volume_bits.store(v.to_bits(), Ordering::Relaxed);
        *self.last_activity.lock() = Instant::now();
    }

    /// Starts recording 10 seconds of raw and cleaned audio for delayed comparison.
    pub fn start_recording(&self) -> Result<(), String> {
        if !self.is_active.load(Ordering::Relaxed) {
            return Err("Live Test must be active to record test audio.".to_string());
        }
        self.record_state.lock().clear();
        self.recording_count.store(0, Ordering::Relaxed);
        self.is_recording.store(true, Ordering::Relaxed);
        *self.last_activity.lock() = Instant::now();
        Ok(())
    }

    /// Starts playback of the recorded 10 seconds (Original followed by Cleaned).
    pub fn start_replay(&mut self) -> Result<(), String> {
        if !self.is_active.load(Ordering::Relaxed) {
            return Err("Live Test must be running to play back recording.".to_string());
        }

        let (raw_len, clean_len) = {
            let rec = self.record_state.lock();
            (rec.raw_samples.len(), rec.clean_samples.len())
        };

        if raw_len == 0 || clean_len == 0 {
            return Err("No recorded audio available. Please record audio first.".to_string());
        }

        self.replay_cursor.store(0, Ordering::Relaxed);
        self.replay_is_cleaned.store(false, Ordering::Relaxed);
        self.is_replaying.store(true, Ordering::Relaxed);
        *self.last_activity.lock() = Instant::now();

        Ok(())
    }

    /// Exports the recorded raw and cleaned audio into WAV files.
    pub fn export_recorded_wavs(&self, output_dir: &Path) -> Result<(PathBuf, PathBuf), String> {
        let (raw, clean) = {
            let rec = self.record_state.lock();
            (rec.raw_samples.clone(), rec.clean_samples.clone())
        };

        if raw.is_empty() || clean.is_empty() {
            return Err("No recorded audio available to export.".to_string());
        }

        std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
        let raw_path = output_dir.join("live_test_original_raw.wav");
        let clean_path = output_dir.join("live_test_noise_cancelled.wav");

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: Self::SAMPLE_RATE_48K,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        // Write raw wav
        {
            let mut writer = hound::WavWriter::create(&raw_path, spec).map_err(|e| e.to_string())?;
            for &s in &raw {
                let sample_i16 = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                writer.write_sample(sample_i16).map_err(|e| e.to_string())?;
            }
            writer.finalize().map_err(|e| e.to_string())?;
        }

        // Write clean wav
        {
            let mut writer = hound::WavWriter::create(&clean_path, spec).map_err(|e| e.to_string())?;
            for &s in &clean {
                let sample_i16 = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                writer.write_sample(sample_i16).map_err(|e| e.to_string())?;
            }
            writer.finalize().map_err(|e| e.to_string())?;
        }

        Ok((raw_path, clean_path))
    }

    /// Returns current status for UI updates and checks for 10-minute inactivity auto-stop.
    pub fn get_status(&mut self) -> LiveMonitorStatus {
        let is_active = self.is_active.load(Ordering::Relaxed);
        let ab_val = self.ab_mode.load(Ordering::Relaxed);
        let volume = f32::from_bits(self.volume_bits.load(Ordering::Relaxed));
        let feedback_detected = self.feedback_detected.load(Ordering::Relaxed);
        let is_recording = self.is_recording.load(Ordering::Relaxed);
        let rec_count = self.recording_count.load(Ordering::Relaxed);
        let is_replaying = self.is_replaying.load(Ordering::Relaxed);
        let replay_is_cleaned = self.replay_is_cleaned.load(Ordering::Relaxed);

        // Auto-stop after 10 minutes (600 seconds) of inactivity
        if is_active && self.last_activity.lock().elapsed().as_secs() > 600 {
            tracing::info!("Auto-stopping Live Test after 10 minutes of inactivity");
            self.stop();
        }

        let has_recorded_audio = {
            let rec = self.record_state.lock();
            !rec.raw_samples.is_empty()
        };

        LiveMonitorStatus {
            is_active: self.is_active.load(Ordering::Relaxed),
            ab_mode: if ab_val == 1 { "Original".to_string() } else { "Cleaned".to_string() },
            volume,
            feedback_detected,
            is_recording,
            recording_seconds: rec_count as f32 / Self::SAMPLE_RATE_48K as f32,
            is_replaying,
            replay_is_cleaned,
            has_recorded_audio,
            selected_device: self.selected_device_name.lock().clone(),
            error_message: self.last_error.lock().clone(),
        }
    }
}

impl Drop for LiveMonitorController {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_monitor_initial_state() {
        let (mut controller, _) = LiveMonitorController::new();
        let status = controller.get_status();
        assert!(!status.is_active);
        assert_eq!(status.ab_mode, "Cleaned");
        assert_eq!(status.volume, 0.50);
        assert!(!status.feedback_detected);
        assert!(!status.is_recording);
        assert!(!status.is_replaying);
    }

    #[test]
    fn test_live_monitor_ab_mode_switching() {
        let (mut controller, _) = LiveMonitorController::new();

        controller.set_ab_mode(CrossfadeMode::Original);
        let s1 = controller.get_status();
        assert_eq!(s1.ab_mode, "Original");

        controller.set_ab_mode(CrossfadeMode::Cleaned);
        let s2 = controller.get_status();
        assert_eq!(s2.ab_mode, "Cleaned");
    }

    #[test]
    fn test_live_monitor_volume_bounds() {
        let (mut controller, _) = LiveMonitorController::new();

        controller.set_volume(0.85);
        assert!((controller.get_status().volume - 0.85).abs() < 1e-4);

        // Clamp tests
        controller.set_volume(1.5);
        assert_eq!(controller.get_status().volume, 1.0);

        controller.set_volume(-0.5);
        assert_eq!(controller.get_status().volume, 0.0);
    }

    #[test]
    fn test_live_monitor_record_state_bounds() {
        let mut rec = RecordReplayState::new(48000);
        assert_eq!(rec.raw_samples.len(), 0);

        let chunk = vec![0.1f32; 480];
        rec.raw_samples.extend_from_slice(&chunk);
        rec.clean_samples.extend_from_slice(&chunk);

        assert_eq!(rec.raw_samples.len(), 480);
        assert_eq!(rec.clean_samples.len(), 480);

        rec.clear();
        assert_eq!(rec.raw_samples.len(), 0);
    }
}
