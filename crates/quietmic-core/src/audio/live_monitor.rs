use crate::audio::device::DeviceManager;
use crate::dsp::crossfade::{CrossfadeMode, CrossfadeSwitch};
use crate::dsp::howling::HowlingDetector;
use crate::dsp::limiter::SoftLimiter;
use crate::dsp::resampler::AudioResampler;
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Status summary of the Live Test monitoring system reported to UI and IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveMonitorStatus {
    pub is_active: bool,
    pub ab_mode: String, // "Cleaned" or "Original"
    pub volume: f32,     // 0.0 to 1.0
    pub feedback_detected: bool,
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

/// Real-time Live Test Monitor Controller.
/// Manages the monitor output stream to headphones, A/B crossfading,
/// volume scaling, ear-protection soft limiting, and feedback howling detection.
pub struct LiveMonitorController {
    is_active: Arc<AtomicBool>,
    ab_mode: Arc<AtomicU8>,       // 0 = Cleaned, 1 = Original
    volume_bits: Arc<AtomicU32>,   // f32::to_bits
    feedback_detected: Arc<AtomicBool>,
    last_activity: Arc<parking_lot::Mutex<Instant>>,
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
    pub const SAMPLE_RATE_48K: u32 = 48000;

    pub fn new() -> (Self, LiveMonitorTapProducers) {
        let is_active = Arc::new(AtomicBool::new(false));
        let ab_mode = Arc::new(AtomicU8::new(0)); // Cleaned by default
        let volume_bits = Arc::new(AtomicU32::new(0.50f32.to_bits())); // 50% volume default
        let feedback_detected = Arc::new(AtomicBool::new(false));
        let last_activity = Arc::new(parking_lot::Mutex::new(Instant::now()));
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
            last_activity,
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
        let dev = device_manager.get_monitoring_output_device(output_device_name)?;
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
        let sample_format = config.sample_format();

        // 3. Setup DSP helpers
        let mut crossfade = CrossfadeSwitch::new(Self::SAMPLE_RATE_48K as f32, 15.0);
        let mut howling_detector = HowlingDetector::default_48k();
        let mut limiter = SoftLimiter::default_preset();

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
        let last_error_cb = Arc::clone(&self.last_error);
        let raw_cons_cb = Arc::clone(&self.raw_cons);
        let clean_cons_cb = Arc::clone(&self.clean_cons);

        const MAX_SCRATCH: usize = 4096;
        let mut scratch_raw = vec![0.0f32; MAX_SCRATCH];
        let mut scratch_clean = vec![0.0f32; MAX_SCRATCH];
        let mut scratch_blended = vec![0.0f32; MAX_SCRATCH];
        let mut scratch_resampled = Vec::with_capacity(MAX_SCRATCH * 2);

        // 4. Build Output Stream
        let err_cb_active = Arc::clone(&self.is_active);
        let err_cb_error = Arc::clone(&self.last_error);

        const PRE_ROLL_SAMPLES: usize = 960; // 20 ms cushion at 48 kHz
        let mut primed_i16 = false;
        let mut primed_f32 = false;

        let stream = match sample_format {
            cpal::SampleFormat::I16 => {
                dev.build_output_stream(
                    &config.into(),
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        if !is_active_cb.load(Ordering::Relaxed) {
                            data.fill(0);
                            return;
                        }

                        let volume = f32::from_bits(volume_bits_cb.load(Ordering::Relaxed));

                        let num_frames_needed = (data.len() / channels).min(MAX_SCRATCH);
                        let target_ab = if ab_mode_cb.load(Ordering::Relaxed) == 1 {
                            CrossfadeMode::Original
                        } else {
                            CrossfadeMode::Cleaned
                        };
                        crossfade.set_mode(target_ab);

                        let input_needed = if resampler.is_some() {
                            ((num_frames_needed as f64 * (Self::SAMPLE_RATE_48K as f64 / target_sample_rate as f64)).ceil() as usize).clamp(1, MAX_SCRATCH)
                        } else {
                            num_frames_needed
                        };

                        let mut r_lock = raw_cons_cb.lock();
                        let mut c_lock = clean_cons_cb.lock();
                        let avail = r_lock.occupied_len().min(c_lock.occupied_len());

                        if !primed_i16 {
                            if avail >= PRE_ROLL_SAMPLES {
                                primed_i16 = true;
                            } else {
                                data.fill(0);
                                return;
                            }
                        }

                        // Cap buffer overrun gently without causing massive 80ms audio jumps
                        if avail > 4800 {
                            let to_drop = (avail - 1920).min(input_needed);
                            for _ in 0..to_drop {
                                let _ = r_lock.try_pop();
                                let _ = c_lock.try_pop();
                            }
                        }

                        let avail_now = r_lock.occupied_len().min(c_lock.occupied_len());
                        let to_read = input_needed.min(avail_now);
                        for i in 0..to_read {
                            scratch_raw[i] = r_lock.try_pop().unwrap_or(0.0);
                            scratch_clean[i] = c_lock.try_pop().unwrap_or(0.0);
                        }
                        drop(r_lock);
                        drop(c_lock);

                        if to_read < input_needed {
                            scratch_raw[to_read..input_needed].fill(0.0);
                            scratch_clean[to_read..input_needed].fill(0.0);
                            // Only un-prime if buffer is completely dry (to_read == 0)
                            // This eliminates 33 Hz buffer flapping and vocal chopping!
                            if to_read == 0 {
                                primed_i16 = false;
                            }
                        }

                        crossfade.process_frame(
                            &scratch_raw[..input_needed],
                            &scratch_clean[..input_needed],
                            &mut scratch_blended[..input_needed],
                        );

                        if howling_detector.detect(&scratch_blended[..input_needed]) {
                            feedback_cb.store(true, Ordering::Relaxed);
                            *last_error_cb.lock() = Some(
                                "Acoustic feedback (howling) detected! Monitor was automatically muted to protect your hearing. Please use headphones.".to_string()
                            );
                            scratch_blended[..input_needed].fill(0.0);
                        } else if feedback_cb.load(Ordering::Relaxed) {
                            feedback_cb.store(false, Ordering::Relaxed);
                            *last_error_cb.lock() = None;
                        }

                        for s in scratch_blended[..input_needed].iter_mut() {
                            *s *= volume;
                        }
                        limiter.process_slice(&mut scratch_blended[..input_needed]);

                        let out_slice: &[f32] = if let Some(ref mut rs) = resampler {
                            scratch_resampled.clear();
                            if rs.process(&scratch_blended[..input_needed], &mut scratch_resampled).is_ok() {
                                &scratch_resampled
                            } else {
                                &scratch_blended[..input_needed]
                            }
                        } else {
                            &scratch_blended[..input_needed]
                        };

                        for (out_idx, frame) in data.chunks_exact_mut(channels).enumerate() {
                            let sample_f = if out_idx < out_slice.len() {
                                out_slice[out_idx]
                            } else {
                                0.0
                            };
                            let sample_i16 = (sample_f.clamp(-1.0, 1.0) * 32767.0) as i16;
                            for ch in frame.iter_mut() {
                                *ch = sample_i16;
                            }
                        }
                    },
                    move |err| {
                        tracing::warn!("Monitor playback stream error: {}", err);
                        err_cb_active.store(false, Ordering::Relaxed);
                        *err_cb_error.lock() = Some(format!("Monitor device error: {}", err));
                    },
                    None,
                )
            }
            _ => {
                dev.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        if !is_active_cb.load(Ordering::Relaxed) {
                            data.fill(0.0);
                            return;
                        }

                        let volume = f32::from_bits(volume_bits_cb.load(Ordering::Relaxed));

                        // Live Capture Monitor Audio (continuous streaming)
                        let num_frames_needed = (data.len() / channels).min(MAX_SCRATCH);

                        let target_ab = if ab_mode_cb.load(Ordering::Relaxed) == 1 {
                            CrossfadeMode::Original
                        } else {
                            CrossfadeMode::Cleaned
                        };
                        crossfade.set_mode(target_ab);

                        let input_needed = if resampler.is_some() {
                            ((num_frames_needed as f64 * (Self::SAMPLE_RATE_48K as f64 / target_sample_rate as f64)).ceil() as usize).clamp(1, MAX_SCRATCH)
                        } else {
                            num_frames_needed
                        };

                        let mut r_lock = raw_cons_cb.lock();
                        let mut c_lock = clean_cons_cb.lock();
                        let avail = r_lock.occupied_len().min(c_lock.occupied_len());

                        if !primed_f32 {
                            if avail >= PRE_ROLL_SAMPLES {
                                primed_f32 = true;
                            } else {
                                data.fill(0.0);
                                return;
                            }
                        }

                        // Cap buffer overrun gently without causing massive 80ms audio jumps
                        if avail > 4800 {
                            let to_drop = (avail - 1920).min(input_needed);
                            for _ in 0..to_drop {
                                let _ = r_lock.try_pop();
                                let _ = c_lock.try_pop();
                            }
                        }

                        let avail_now = r_lock.occupied_len().min(c_lock.occupied_len());
                        let to_read = input_needed.min(avail_now);
                        for i in 0..to_read {
                            scratch_raw[i] = r_lock.try_pop().unwrap_or(0.0);
                            scratch_clean[i] = c_lock.try_pop().unwrap_or(0.0);
                        }
                        drop(r_lock);
                        drop(c_lock);

                        if to_read < input_needed {
                            scratch_raw[to_read..input_needed].fill(0.0);
                            scratch_clean[to_read..input_needed].fill(0.0);
                            // Only un-prime if buffer is completely dry (to_read == 0)
                            // This eliminates 33 Hz buffer flapping and vocal chopping!
                            if to_read == 0 {
                                primed_f32 = false;
                            }
                        }

                        // Perform smooth click-free A/B crossfade
                        crossfade.process_frame(
                            &scratch_raw[..input_needed],
                            &scratch_clean[..input_needed],
                            &mut scratch_blended[..input_needed],
                        );

                        // Run Howling / Acoustic Feedback Detection
                        if howling_detector.detect(&scratch_blended[..input_needed]) {
                            feedback_cb.store(true, Ordering::Relaxed);
                            *last_error_cb.lock() = Some(
                                "Acoustic feedback (howling) detected! Monitor was automatically muted to protect your hearing. Please use headphones.".to_string()
                            );
                            scratch_blended[..input_needed].fill(0.0);
                        } else if feedback_cb.load(Ordering::Relaxed) {
                            feedback_cb.store(false, Ordering::Relaxed);
                            *last_error_cb.lock() = None;
                        }

                        // Apply volume scaling and ear-protection soft limiter
                        for s in scratch_blended[..input_needed].iter_mut() {
                            *s *= volume;
                        }
                        limiter.process_slice(&mut scratch_blended[..input_needed]);

                        // Resample to device native rate if needed
                        let out_slice: &[f32] = if let Some(ref mut rs) = resampler {
                            scratch_resampled.clear();
                            if rs.process(&scratch_blended[..input_needed], &mut scratch_resampled).is_ok() {
                                &scratch_resampled
                            } else {
                                &scratch_blended[..input_needed]
                            }
                        } else {
                            &scratch_blended[..input_needed]
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
                )
            }
        }.map_err(|e| format!("Failed to create monitor playback stream: {}", e))?;

        stream.play().map_err(|e| format!("Failed to start monitor stream: {}", e))?;
        self.stream = Some(stream);
        self.is_active.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// Stops live monitoring and drops the audio stream immediately.
    pub fn stop(&mut self) {
        self.is_active.store(false, Ordering::Relaxed);
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

    /// Returns current status for UI updates.
    pub fn get_status(&mut self) -> LiveMonitorStatus {
        let is_active = self.is_active.load(Ordering::Relaxed);
        let ab_val = self.ab_mode.load(Ordering::Relaxed);
        let volume = f32::from_bits(self.volume_bits.load(Ordering::Relaxed));
        let feedback_detected = self.feedback_detected.load(Ordering::Relaxed);

        LiveMonitorStatus {
            is_active,
            ab_mode: if ab_val == 1 { "Original".to_string() } else { "Cleaned".to_string() },
            volume,
            feedback_detected,
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
}
