use crate::audio::device::DeviceManager;
use crate::audio::mmcss::MmcssHandle;
use crate::engine::pipeline::AudioPipeline;
use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Atomic real-time monitoring metrics shared between audio thread and UI.
pub struct AudioMetrics {
    pub frame_time_us: AtomicU64,
    pub input_peak_db: AtomicU32, // Bitwise float representation
    pub output_peak_db: AtomicU32,
    pub underruns: AtomicU64,
    pub overruns: AtomicU64,
}

impl AudioMetrics {
    pub fn new() -> Self {
        Self {
            frame_time_us: AtomicU64::new(0),
            input_peak_db: AtomicU32::new((-100.0f32).to_bits()),
            output_peak_db: AtomicU32::new((-100.0f32).to_bits()),
            underruns: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
        }
    }

    pub fn get_frame_time_us(&self) -> u64 {
        self.frame_time_us.load(Ordering::Relaxed)
    }

    pub fn get_input_peak_db(&self) -> f32 {
        f32::from_bits(self.input_peak_db.load(Ordering::Relaxed))
    }

    pub fn get_output_peak_db(&self) -> f32 {
        f32::from_bits(self.output_peak_db.load(Ordering::Relaxed))
    }
}

impl Default for AudioMetrics {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AudioStreamController {
    running: Arc<AtomicBool>,
    metrics: Arc<AudioMetrics>,
    worker_handle: Option<JoinHandle<()>>,
    _capture_stream: Option<cpal::Stream>,
    _render_stream: Option<cpal::Stream>,
}

// Safe FFI/thread boundary: on Windows WASAPI, the underlying cpal::Stream
// handles are thread-safe and can be transferred/dropped across worker threads.
unsafe impl Send for AudioStreamController {}
unsafe impl Sync for AudioStreamController {}

impl AudioStreamController {
    /// Buffer capacity in samples (~100 ms headroom for lock-free SPSC queues).
    pub const RB_CAPACITY: usize = 4800;

    /// Starts real-time capture from mic, processing pipeline in MMCSS Pro Audio thread,
    /// and output to VB-CABLE virtual microphone.
    pub fn start(
        device_manager: &DeviceManager,
        selected_mic_name: Option<&str>,
        pipeline: AudioPipeline,
    ) -> Result<Self, String> {
        let running = Arc::new(AtomicBool::new(true));
        let metrics = Arc::new(AudioMetrics::new());

        let mic_device = device_manager.get_input_device(selected_mic_name)?;
        let mic_name = mic_device.name().unwrap_or_else(|_| "Default Microphone".to_string());
        tracing::info!("Selected input microphone: {}", mic_name);

        let mic_config = mic_device.default_input_config()
            .map_err(|e| format!("Failed to read mic config: {}", e))?;
        let _mic_sample_rate = mic_config.sample_rate().0;
        let mic_channels = mic_config.channels() as usize;

        // Try getting VB-CABLE Input (render device)
        let cable_device_res = device_manager.get_virtual_cable_input();
        let (cable_device, _cable_sample_rate, cable_channels) = match cable_device_res {
            Ok(dev) => {
                let name = dev.name().unwrap_or_else(|_| "VB-CABLE".to_string());
                tracing::info!("Connected to virtual microphone render: {}", name);
                let cfg = dev.default_output_config()
                    .map_err(|e| format!("Failed to read virtual cable config: {}", e))?;
                let sr = cfg.sample_rate().0;
                let ch = cfg.channels() as usize;
                (Some(dev), sr, ch)
            }
            Err(e) => {
                tracing::warn!("Virtual mic not connected: {}. Audio will run in loopback/standby mode.", e);
                (None, 48000, 2)
            }
        };

        // Create lock-free ring buffers
        let input_rb = HeapRb::<f32>::new(Self::RB_CAPACITY);
        let output_rb = HeapRb::<f32>::new(Self::RB_CAPACITY);
        let (mut in_prod, mut in_cons) = input_rb.split();
        let (mut out_prod, mut out_cons) = output_rb.split();

        // 1. Microphone Capture Stream Callback
        let is_running_capture = Arc::clone(&running);
        let capture_stream = mic_device.build_input_stream(
            &mic_config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if !is_running_capture.load(Ordering::Relaxed) {
                    return;
                }
                // Downmix to mono if multi-channel
                if mic_channels > 1 {
                    for frame in data.chunks_exact(mic_channels) {
                        let mono = frame.iter().sum::<f32>() / mic_channels as f32;
                        let _ = in_prod.try_push(mono);
                    }
                } else {
                    for &sample in data {
                        let _ = in_prod.try_push(sample);
                    }
                }
            },
            |err| tracing::error!("Microphone capture stream error: {}", err),
            None,
        ).map_err(|e| format!("Failed to build mic capture stream: {}", e))?;

        capture_stream.play().map_err(|e| format!("Failed to start capture stream: {}", e))?;

        // 2. Virtual Cable Output Render Stream Callback
        let is_running_render = Arc::clone(&running);
        let render_stream = if let Some(ref dev) = cable_device {
            let out_cfg = dev.default_output_config()
                .map_err(|e| format!("Failed to get output config: {}", e))?;

            let stream = dev.build_output_stream(
                &out_cfg.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if !is_running_render.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        return;
                    }
                    if cable_channels > 1 {
                        for frame in data.chunks_exact_mut(cable_channels) {
                            let sample = out_cons.try_pop().unwrap_or(0.0);
                            for ch in frame.iter_mut() {
                                *ch = sample;
                            }
                        }
                    } else {
                        for sample in data.iter_mut() {
                            *sample = out_cons.try_pop().unwrap_or(0.0);
                        }
                    }
                },
                |err| tracing::error!("Virtual mic render stream error: {}", err),
                None,
            ).map_err(|e| format!("Failed to build render stream: {}", e))?;

            stream.play().map_err(|e| format!("Failed to start render stream: {}", e))?;
            Some(stream)
        } else {
            None
        };

        // 3. Dedicated Audio Worker Thread (MMCSS Pro Audio)
        let is_running_worker = Arc::clone(&running);
        let worker_metrics = Arc::clone(&metrics);
        let mut worker_pipeline = pipeline;

        let worker_handle = std::thread::Builder::new()
            .name("quietmic-audio-dsp".to_string())
            .spawn(move || {
                // Register MMCSS "Pro Audio" priority
                let _mmcss = MmcssHandle::register_pro_audio();

                let mut in_chunk = vec![0.0f32; AudioPipeline::FRAME_SAMPLES];
                let mut out_chunk = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES + 10);

                while is_running_worker.load(Ordering::Relaxed) {
                    if in_cons.occupied_len() >= AudioPipeline::FRAME_SAMPLES {
                        // Read exactly 1 frame (10 ms = 480 samples)
                        for sample in in_chunk.iter_mut() {
                            *sample = in_cons.try_pop().unwrap_or(0.0);
                        }

                        // Measure peak input dB
                        let mut in_peak = 0.0f32;
                        for &s in &in_chunk {
                            in_peak = in_peak.max(s.abs());
                        }
                        let in_db = if in_peak > 1e-5 { 20.0 * in_peak.log10() } else { -100.0 };
                        worker_metrics.input_peak_db.store(in_db.to_bits(), Ordering::Relaxed);

                        // High resolution frame processing timing
                        let t0 = std::time::Instant::now();
                        let current_fill = out_prod.occupied_len();

                        let res = worker_pipeline.process_frame_internal(
                            &in_chunk,
                            &mut out_chunk,
                            current_fill,
                        );

                        let elapsed_us = t0.elapsed().as_micros() as u64;
                        worker_metrics.frame_time_us.store(elapsed_us, Ordering::Relaxed);

                        if res.is_ok() {
                            let mut out_peak = 0.0f32;
                            for &s in &out_chunk {
                                out_peak = out_peak.max(s.abs());
                                if out_prod.try_push(s).is_err() {
                                    worker_metrics.overruns.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            let out_db = if out_peak > 1e-5 { 20.0 * out_peak.log10() } else { -100.0 };
                            worker_metrics.output_peak_db.store(out_db.to_bits(), Ordering::Relaxed);
                        }
                    } else {
                        // Sleep briefly (~1 ms) to prevent high CPU burn while waiting for audio chunk
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }

                tracing::info!("Audio DSP thread cleanly terminated");
            })
            .map_err(|e| format!("Failed to spawn audio DSP thread: {}", e))?;

        Ok(Self {
            running,
            metrics,
            worker_handle: Some(worker_handle),
            _capture_stream: Some(capture_stream),
            _render_stream: render_stream,
        })
    }

    pub fn metrics(&self) -> &AudioMetrics {
        &self.metrics
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AudioStreamController {
    fn drop(&mut self) {
        self.stop();
    }
}
