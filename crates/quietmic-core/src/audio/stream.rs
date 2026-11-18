use crate::audio::device::DeviceManager;
use crate::audio::echo_doctor::{EchoDoctor, EndpointType};
use crate::audio::live_monitor::LiveMonitorTapProducers;
use crate::audio::mmcss::MmcssHandle;
use crate::dsp::AudioResampler;
use crate::engine::pipeline::{AudioPipeline, EngineMode};
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
    pub frames_received: AtomicU64,
    pub frames_processed: AtomicU64,
    pub frames_passthrough: AtomicU64,
    pub fallback_active: AtomicBool,
    pub noise_reduction_db: AtomicU32, // Bitwise float representation
    pub aec_enabled: AtomicBool,
    pub aec_ref_device_name: std::sync::RwLock<String>,
    pub aec_ref_sample_rate: AtomicU32,
    pub aec_ref_peak_db: AtomicU32,
    pub aec_frames_processed: AtomicU64,
    pub aec_erle_db: AtomicU32,
    pub aec_delay_ms: AtomicU32,
    pub aec_endpoint_type: AtomicU32, // 0 = Unknown, 1 = Headphones, 2 = Headset, 3 = Speakers, 4 = Bluetooth
    pub loop_warning_active: AtomicBool,
    pub raw_waveform: [AtomicU32; Self::WAVEFORM_SNAPSHOT_LEN],
    pub clean_waveform: [AtomicU32; Self::WAVEFORM_SNAPSHOT_LEN],
}

impl AudioMetrics {
    pub const WAVEFORM_SNAPSHOT_LEN: usize = 32;

    pub fn new() -> Self {
        Self {
            frame_time_us: AtomicU64::new(0),
            input_peak_db: AtomicU32::new((-100.0f32).to_bits()),
            output_peak_db: AtomicU32::new((-100.0f32).to_bits()),
            underruns: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
            frames_received: AtomicU64::new(0),
            frames_processed: AtomicU64::new(0),
            frames_passthrough: AtomicU64::new(0),
            fallback_active: AtomicBool::new(false),
            noise_reduction_db: AtomicU32::new(0.0f32.to_bits()),
            aec_enabled: AtomicBool::new(false),
            aec_ref_device_name: std::sync::RwLock::new("None".to_string()),
            aec_ref_sample_rate: AtomicU32::new(0),
            aec_ref_peak_db: AtomicU32::new((-100.0f32).to_bits()),
            aec_frames_processed: AtomicU64::new(0),
            aec_erle_db: AtomicU32::new(0.0f32.to_bits()),
            aec_delay_ms: AtomicU32::new(0.0f32.to_bits()),
            aec_endpoint_type: AtomicU32::new(0),
            loop_warning_active: AtomicBool::new(false),
            raw_waveform: std::array::from_fn(|_| AtomicU32::new(0)),
            clean_waveform: std::array::from_fn(|_| AtomicU32::new(0)),
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

    pub fn get_frames_received(&self) -> u64 {
        self.frames_received.load(Ordering::Relaxed)
    }

    pub fn get_frames_processed(&self) -> u64 {
        self.frames_processed.load(Ordering::Relaxed)
    }

    pub fn get_frames_passthrough(&self) -> u64 {
        self.frames_passthrough.load(Ordering::Relaxed)
    }

    pub fn is_fallback_active(&self) -> bool {
        self.fallback_active.load(Ordering::Relaxed)
    }

    pub fn get_noise_reduction_db(&self) -> f32 {
        f32::from_bits(self.noise_reduction_db.load(Ordering::Relaxed))
    }

    pub fn is_aec_enabled(&self) -> bool {
        self.aec_enabled.load(Ordering::Relaxed)
    }

    pub fn get_aec_ref_device_name(&self) -> String {
        self.aec_ref_device_name.read().map(|g| g.clone()).unwrap_or_else(|_| "Unknown".to_string())
    }

    pub fn get_aec_ref_sample_rate(&self) -> u32 {
        self.aec_ref_sample_rate.load(Ordering::Relaxed)
    }

    pub fn get_aec_ref_peak_db(&self) -> f32 {
        f32::from_bits(self.aec_ref_peak_db.load(Ordering::Relaxed))
    }

    pub fn get_aec_frames_processed(&self) -> u64 {
        self.aec_frames_processed.load(Ordering::Relaxed)
    }

    pub fn get_aec_erle_db(&self) -> f32 {
        f32::from_bits(self.aec_erle_db.load(Ordering::Relaxed))
    }

    pub fn get_aec_delay_ms(&self) -> f32 {
        f32::from_bits(self.aec_delay_ms.load(Ordering::Relaxed))
    }

    pub fn get_aec_endpoint_type(&self) -> &'static str {
        match self.aec_endpoint_type.load(Ordering::Relaxed) {
            1 => "Headphones",
            2 => "Headset",
            3 => "Speakers",
            4 => "Bluetooth",
            _ => "Unknown",
        }
    }

    pub fn is_loop_warning_active(&self) -> bool {
        self.loop_warning_active.load(Ordering::Relaxed)
    }

    pub fn update_waveform_snapshot(&self, raw: &[f32], clean: &[f32]) {
        let step_raw = (raw.len() / Self::WAVEFORM_SNAPSHOT_LEN).max(1);
        for (i, cell) in self.raw_waveform.iter().enumerate() {
            let idx = (i * step_raw).min(raw.len().saturating_sub(1));
            let s = if idx < raw.len() { raw[idx] } else { 0.0 };
            cell.store(s.to_bits(), Ordering::Relaxed);
        }
        let step_clean = (clean.len() / Self::WAVEFORM_SNAPSHOT_LEN).max(1);
        for (i, cell) in self.clean_waveform.iter().enumerate() {
            let idx = (i * step_clean).min(clean.len().saturating_sub(1));
            let s = if idx < clean.len() { clean[idx] } else { 0.0 };
            cell.store(s.to_bits(), Ordering::Relaxed);
        }
    }

    pub fn get_waveform_snapshot(&self) -> (Vec<f32>, Vec<f32>) {
        let mut raw = Vec::with_capacity(Self::WAVEFORM_SNAPSHOT_LEN);
        let mut clean = Vec::with_capacity(Self::WAVEFORM_SNAPSHOT_LEN);
        for cell in &self.raw_waveform {
            raw.push(f32::from_bits(cell.load(Ordering::Relaxed)));
        }
        for cell in &self.clean_waveform {
            clean.push(f32::from_bits(cell.load(Ordering::Relaxed)));
        }
        (raw, clean)
    }
}

impl Default for AudioMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct AudioStreamControls {
    pub mic_volume_bits: Arc<AtomicU32>,
    pub ptt_enabled: Arc<AtomicBool>,
    pub ptt_active: Arc<AtomicBool>,
    pub auto_sensitivity: Arc<AtomicBool>,
    pub sensitivity_threshold_dbfs: Arc<AtomicU32>,
    pub aec_enabled: Arc<AtomicBool>,
}

impl Default for AudioStreamControls {
    fn default() -> Self {
        Self {
            mic_volume_bits: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            ptt_enabled: Arc::new(AtomicBool::new(false)),
            ptt_active: Arc::new(AtomicBool::new(false)),
            auto_sensitivity: Arc::new(AtomicBool::new(true)),
            sensitivity_threshold_dbfs: Arc::new(AtomicU32::new((-50.0f32).to_bits())),
            aec_enabled: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct AudioStreamController {
    running: Arc<AtomicBool>,
    metrics: Arc<AudioMetrics>,
    controls: AudioStreamControls,
    worker_handle: Option<JoinHandle<()>>,
    _capture_stream: Option<cpal::Stream>,
    _render_stream: Option<cpal::Stream>,
    _ref_stream: Option<cpal::Stream>,
    _speaker_controller: Option<Box<AudioStreamController>>,
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
        speaker_pipeline: Option<AudioPipeline>,
        mut live_monitor_tap: Option<LiveMonitorTapProducers>,
    ) -> Result<Self, String> {
        let running = Arc::new(AtomicBool::new(true));
        let metrics = Arc::new(AudioMetrics::new());

        let mic_device = device_manager.get_input_device(selected_mic_name)?;
        let mic_name = mic_device.name().unwrap_or_else(|_| "Default Microphone".to_string());
        tracing::info!("Selected input microphone: {}", mic_name);

        let mic_config = mic_device.default_input_config()
            .map_err(|e| format!("Failed to read mic config: {}", e))?;
        let mic_sample_rate = mic_config.sample_rate().0;
        let mic_channels = (mic_config.channels() as usize).max(1);

        // Try getting VB-CABLE Input (render device)
        let cable_device_res = device_manager.get_virtual_cable_input();
        let (cable_device, cable_sample_rate, cable_channels) = match cable_device_res {
            Ok(dev) => {
                let name = dev.name().unwrap_or_else(|_| "VB-CABLE".to_string());
                tracing::info!("Connected to virtual microphone render: {}", name);
                let cfg = dev.default_output_config()
                    .map_err(|e| format!("Failed to read virtual cable config: {}", e))?;
                let sr = cfg.sample_rate().0;
                let ch = (cfg.channels() as usize).max(1);
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
        #[allow(unused_mut)]
        let (mut in_prod, mut in_cons) = input_rb.split();
        #[allow(unused_mut)]
        let (mut out_prod, mut out_cons) = output_rb.split();

        // 1. Microphone Capture Stream Callback (Universal F32 and I16 support)
        let is_running_capture = Arc::clone(&running);
        let capture_stream = match mic_config.sample_format() {
            cpal::SampleFormat::I16 => {
                let mut in_prod = in_prod;
                let is_running = Arc::clone(&is_running_capture);
                mic_device.build_input_stream(
                    &mic_config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if !is_running.load(Ordering::Relaxed) {
                            return;
                        }
                        if mic_channels > 1 {
                            for frame in data.chunks_exact(mic_channels) {
                                let mono = frame.iter().map(|&s| s as f32).sum::<f32>() / (mic_channels as f32 * 32768.0);
                                let _ = in_prod.try_push(mono);
                            }
                        } else {
                            for &sample in data {
                                let mono = (sample as f32) / 32768.0;
                                let _ = in_prod.try_push(mono);
                            }
                        }
                    },
                    |err| tracing::error!("Microphone capture stream error: {}", err),
                    None,
                )
            }
            _ => {
                let mut in_prod = in_prod;
                let is_running = Arc::clone(&is_running_capture);
                mic_device.build_input_stream(
                    &mic_config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !is_running.load(Ordering::Relaxed) {
                            return;
                        }
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
                )
            }
        }.map_err(|e| format!("Failed to build mic capture stream: {}", e))?;

        capture_stream.play().map_err(|e| format!("Failed to start capture stream: {}", e))?;

        // 2. Virtual Cable Output Render Stream Callback (Universal F32 and I16 support)
        let is_running_render = Arc::clone(&running);
        let render_stream = if let Some(ref dev) = cable_device {
            let out_cfg = dev.default_output_config()
                .map_err(|e| format!("Failed to get output config: {}", e))?;

            let stream = match out_cfg.sample_format() {
                cpal::SampleFormat::I16 => {
                    let mut out_cons = out_cons;
                    let is_running = Arc::clone(&is_running_render);
                    dev.build_output_stream(
                        &out_cfg.into(),
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            if !is_running.load(Ordering::Relaxed) {
                                data.fill(0);
                                return;
                            }
                            if cable_channels > 1 {
                                for frame in data.chunks_exact_mut(cable_channels) {
                                    let sample_f = out_cons.try_pop().unwrap_or(0.0);
                                    let sample_i = (sample_f.clamp(-1.0, 1.0) * 32767.0) as i16;
                                    for ch in frame.iter_mut() {
                                        *ch = sample_i;
                                    }
                                }
                            } else {
                                for sample in data.iter_mut() {
                                    let sample_f = out_cons.try_pop().unwrap_or(0.0);
                                    *sample = (sample_f.clamp(-1.0, 1.0) * 32767.0) as i16;
                                }
                            }
                        },
                        |err| tracing::error!("Virtual mic render stream error: {}", err),
                        None,
                    )
                }
                _ => {
                    let mut out_cons = out_cons;
                    let is_running = Arc::clone(&is_running_render);
                    dev.build_output_stream(
                        &out_cfg.into(),
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            if !is_running.load(Ordering::Relaxed) {
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
                    )
                }
            }.map_err(|e| format!("Failed to build render stream: {}", e))?;

            stream.play().map_err(|e| format!("Failed to start render stream: {}", e))?;
            Some(stream)
        } else {
            None
        };

        // 3. AEC Reference Loopback Stream (captures real physical render device audio for echo cancellation)
        // Guard against selecting a virtual cable as reference, which would cause an echo loop!
        let physical_speaker_dev = match device_manager.get_default_output_device() {
            Ok(dev) => {
                let name = dev.name().unwrap_or_else(|_| "Default Output".to_string());
                let lower = name.to_lowercase();
                if lower.contains("cable input") || (lower.contains("cable") && lower.contains("vb-audio")) || lower.contains("quietmic") {
                    tracing::warn!("Default output device is virtual cable ('{}'). AEC loopback must not capture virtual cable!", name);
                    metrics.loop_warning_active.store(true, Ordering::Relaxed);
                    // Search for a real physical device (e.g. Speakers, Headphones)
                    device_manager.get_monitoring_output_device(None).ok()
                } else {
                    metrics.loop_warning_active.store(false, Ordering::Relaxed);
                    Some(dev)
                }
            }
            Err(_) => None,
        };

        let (ref_stream, mut ref_cons) = if let Some(speaker_dev) = physical_speaker_dev {
            let spk_name = cpal::traits::DeviceTrait::name(&speaker_dev).unwrap_or_else(|_| "Default Output".to_string());
            let endpoint = EchoDoctor::detect_endpoint_type(&spk_name);
            let endpoint_code = match endpoint {
                EndpointType::Headphones => 1,
                EndpointType::Headset => 2,
                EndpointType::Speakers => 3,
                EndpointType::Bluetooth => 4,
                EndpointType::Unknown => 0,
            };
            metrics.aec_endpoint_type.store(endpoint_code, Ordering::Relaxed);

            if let Ok(spk_cfg) = speaker_dev.default_output_config() {
                let spk_channels = (spk_cfg.channels() as usize).max(1);
                let spk_sample_rate = spk_cfg.sample_rate().0;
                let ref_rb = HeapRb::<f32>::new(Self::RB_CAPACITY);
                let (mut ref_prod, ref_cons) = ref_rb.split();
                let is_running_ref = Arc::clone(&running);

                if let Ok(mut g) = metrics.aec_ref_device_name.write() {
                    *g = format!("{} ({})", spk_name, endpoint.display_name());
                }
                metrics.aec_ref_sample_rate.store(spk_sample_rate, Ordering::Relaxed);

                let stream_res = speaker_dev.build_input_stream(
                    &spk_cfg.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !is_running_ref.load(Ordering::Relaxed) {
                            return;
                        }
                        if spk_channels > 1 {
                            for frame in data.chunks_exact(spk_channels) {
                                let mono = frame.iter().sum::<f32>() / spk_channels as f32;
                                let _ = ref_prod.try_push(mono);
                            }
                        } else {
                            for &sample in data {
                                let _ = ref_prod.try_push(sample);
                            }
                        }
                    },
                    |err| tracing::warn!("AEC loopback reference stream warning: {}", err),
                    None,
                );

                match stream_res {
                    Ok(st) => {
                        let _ = st.play();
                        tracing::info!("AEC loopback reference capture initialized on: {}", metrics.get_aec_ref_device_name());
                        (Some(st), Some(ref_cons))
                    }
                    Err(e) => {
                        tracing::warn!("Could not start AEC loopback stream: {}", e);
                        (None, None)
                    }
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // 4. Dedicated Audio Worker Thread (MMCSS Pro Audio)
        let controls = AudioStreamControls {
            mic_volume_bits: Arc::new(AtomicU32::new(pipeline.mic_gain().to_bits())),
            ptt_enabled: Arc::new(AtomicBool::new(pipeline.is_push_to_talk())),
            ptt_active: Arc::new(AtomicBool::new(pipeline.is_ptt_active())),
            auto_sensitivity: Arc::new(AtomicBool::new(true)),
            sensitivity_threshold_dbfs: Arc::new(AtomicU32::new(pipeline.sensitivity_threshold_dbfs().to_bits())),
            aec_enabled: Arc::new(AtomicBool::new(pipeline.is_aec_enabled())),
        };
        let is_running_worker = Arc::clone(&running);
        let worker_metrics = Arc::clone(&metrics);
        let worker_controls = controls.clone();
        let mut worker_pipeline = pipeline;
        if let Err(e) = worker_pipeline.update_sample_rates(mic_sample_rate, cable_sample_rate) {
            tracing::warn!("Could not dynamically adapt pipeline resamplers (mic={}, cable={}): {}", mic_sample_rate, cable_sample_rate, e);
        }

        let worker_handle = std::thread::Builder::new()
            .name("180hznc-audio-dsp".to_string())
            .spawn(move || {
                // Register MMCSS "Pro Audio" priority
                let _mmcss = MmcssHandle::register_pro_audio();

                let in_chunk_samples = ((mic_sample_rate as usize) / 100).max(1);
                let mut in_chunk = vec![0.0f32; in_chunk_samples];
                let mut out_chunk = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES + 10);
                let mut ref_scratch = [0.0f32; AudioPipeline::FRAME_SAMPLES];
                let mut ref_resampled = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES * 2);
                let ref_sr = worker_metrics.aec_ref_sample_rate.load(Ordering::Relaxed).max(8000);
                let mut ref_resampler = AudioResampler::new(ref_sr, AudioPipeline::INTERNAL_RATE, AudioPipeline::FRAME_SAMPLES).ok();
                let mut starvation_counter = 0usize;

                while is_running_worker.load(Ordering::Relaxed) {
                    // Sync real-time controls to pipeline lock-free
                    let mic_vol = f32::from_bits(worker_controls.mic_volume_bits.load(Ordering::Relaxed));
                    worker_pipeline.set_mic_gain(mic_vol);

                    let ptt_en = worker_controls.ptt_enabled.load(Ordering::Relaxed);
                    let ptt_act = worker_controls.ptt_active.load(Ordering::Relaxed);
                    worker_pipeline.set_push_to_talk(ptt_en);
                    worker_pipeline.set_ptt_active(ptt_act);

                    let auto_sens = worker_controls.auto_sensitivity.load(Ordering::Relaxed);
                    let sens_thresh = f32::from_bits(worker_controls.sensitivity_threshold_dbfs.load(Ordering::Relaxed));
                    worker_pipeline.set_auto_sensitivity(auto_sens);
                    worker_pipeline.set_sensitivity_threshold(sens_thresh);

                    let aec_en = worker_controls.aec_enabled.load(Ordering::Relaxed);
                    let endpoint_code = worker_metrics.aec_endpoint_type.load(Ordering::Relaxed);
                    let is_headphones = endpoint_code == 1 || endpoint_code == 2;
                    let effective_aec_en = aec_en && !is_headphones;
                    worker_pipeline.set_aec_enabled(effective_aec_en);
                    worker_metrics.aec_enabled.store(aec_en, Ordering::Relaxed);

                    // Drain AEC reference samples if loopback is active
                    if let Some(ref mut r_cons) = ref_cons {
                        let mut count = 0;
                        let mut ref_peak = 0.0f32;
                        while count < AudioPipeline::FRAME_SAMPLES {
                            if let Some(s) = r_cons.try_pop() {
                                ref_scratch[count] = s;
                                let abs_s = s.abs();
                                if abs_s > ref_peak {
                                    ref_peak = abs_s;
                                }
                                count += 1;
                            } else {
                                break;
                            }
                        }

                        // Jitter & Backlog Protection:
                        // Prevent latency accumulation by draining excess unread samples
                        let backlog = r_cons.occupied_len();
                        if backlog > 2 * AudioPipeline::FRAME_SAMPLES {
                            let excess = backlog - AudioPipeline::FRAME_SAMPLES;
                            for _ in 0..excess {
                                let _ = r_cons.try_pop();
                            }
                        }

                        if count > 0 && aec_en {
                            let feed_slice = if let Some(ref mut rs) = ref_resampler {
                                if rs.process(&ref_scratch[..count], &mut ref_resampled).is_ok() && !ref_resampled.is_empty() {
                                    &ref_resampled[..]
                                } else {
                                    &ref_scratch[..count]
                                }
                            } else {
                                &ref_scratch[..count]
                            };
                            if effective_aec_en {
                                worker_pipeline.feed_aec_reference(feed_slice);
                            }
                            let ref_db = if ref_peak > 1e-5 { 20.0 * ref_peak.log10() } else { -100.0 };
                            worker_metrics.aec_ref_peak_db.store(ref_db.to_bits(), Ordering::Relaxed);
                            worker_metrics.aec_frames_processed.fetch_add(1, Ordering::Relaxed);
                            let erle = if effective_aec_en { worker_pipeline.aec().last_erle_db() } else { 0.0 };
                            let delay = if effective_aec_en { worker_pipeline.aec().bulk_delay_ms() } else { 0.0 };
                            worker_metrics.aec_erle_db.store(erle.to_bits(), Ordering::Relaxed);
                            worker_metrics.aec_delay_ms.store(delay.to_bits(), Ordering::Relaxed);
                        } else if !aec_en {
                            worker_metrics.aec_ref_peak_db.store((-100.0f32).to_bits(), Ordering::Relaxed);
                        }
                    }

                    if in_cons.occupied_len() >= in_chunk_samples {
                        // Read exactly 1 frame (10 ms at mic_sample_rate)
                        for sample in in_chunk.iter_mut() {
                            *sample = in_cons.try_pop().unwrap_or(0.0);
                        }

                        // Measure peak input dB
                        let mut in_peak = 0.0f32;
                        for &s in &in_chunk {
                            in_peak = in_peak.max(s.abs());
                        }
                        let in_db = if in_peak > 1e-5 { 20.0 * in_peak.log10() } else { -100.0 };
                        worker_metrics.frames_received.fetch_add(1, Ordering::Relaxed);
                        worker_metrics.input_peak_db.store(in_db.to_bits(), Ordering::Relaxed);

                        // High resolution frame processing timing
                        let t0 = std::time::Instant::now();
                        let current_fill = out_prod.occupied_len();

                        let res = worker_pipeline.process_frame_internal(
                            &in_chunk,
                            &mut out_chunk,
                            current_fill,
                        );

                        if worker_metrics.frames_received.load(Ordering::Relaxed).is_multiple_of(3) {
                            worker_metrics.update_waveform_snapshot(&in_chunk, &out_chunk);
                        }

                        let elapsed_us = t0.elapsed().as_micros() as u64;
                        worker_metrics.frame_time_us.store(elapsed_us, Ordering::Relaxed);

                        // Auto-downgrade under severe CPU starvation to prevent audible dropouts
                        if elapsed_us > 8000 {
                            starvation_counter += 1;
                            if starvation_counter >= 5 && worker_pipeline.mode() != EngineMode::Light {
                                tracing::warn!("CPU starvation detected (>80% frame budget over 5 consecutive frames). Auto-downgrading to Light mode.");
                                worker_pipeline.set_mode(EngineMode::Light);
                                starvation_counter = 0;
                            }
                        } else {
                            starvation_counter = starvation_counter.saturating_sub(1);
                        }

                        match res {
                            Ok(()) => {
                                worker_metrics.frames_processed.fetch_add(1, Ordering::Relaxed);
                                worker_metrics.fallback_active.store(false, Ordering::Relaxed);

                                if let Some(ref mut tap) = live_monitor_tap {
                                    // Provide 48 kHz dry (pre-denoise) and blended (post-Krisp) buffers to live monitor
                                    tap.push_frame(worker_pipeline.dry_buffer(), worker_pipeline.blended_buffer());
                                }
                                let mut out_peak = 0.0f32;
                                for &s in &out_chunk {
                                    out_peak = out_peak.max(s.abs());
                                    if out_prod.try_push(s).is_err() {
                                        worker_metrics.overruns.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                                let out_db = if out_peak > 1e-5 { 20.0 * out_peak.log10() } else { -100.0 };
                                worker_metrics.output_peak_db.store(out_db.to_bits(), Ordering::Relaxed);

                                // Compute real-time noise reduction estimate in dB
                                let nr_db = (in_db - out_db).clamp(0.0, 80.0);
                                worker_metrics.noise_reduction_db.store(nr_db.to_bits(), Ordering::Relaxed);
                            }
                            Err(err_msg) => {
                                tracing::error!("Audio DSP pipeline failure! Silent pass-through active: {}", err_msg);
                                worker_metrics.frames_passthrough.fetch_add(1, Ordering::Relaxed);
                                worker_metrics.fallback_active.store(true, Ordering::Relaxed);
                                worker_metrics.noise_reduction_db.store(0.0f32.to_bits(), Ordering::Relaxed);

                                if let Some(ref mut tap) = live_monitor_tap {
                                    tap.push_frame(worker_pipeline.dry_buffer(), worker_pipeline.dry_buffer());
                                }
                                // Engine/resampler fallback: pass-through unprocessed in_chunk
                                for &s in &in_chunk {
                                    if out_prod.try_push(s).is_err() {
                                        worker_metrics.overruns.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                    } else {
                        // Sleep briefly (~1 ms) to prevent high CPU burn while waiting for audio chunk
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }

                tracing::info!("Audio DSP thread cleanly terminated");
            })
            .map_err(|e| format!("Failed to spawn audio DSP thread: {}", e))?;

        // 5. Optional Speaker Denoise Secondary Stream
        let speaker_controller = if let Some(spk_pipe) = speaker_pipeline {
            match Self::start_speaker_denoise(device_manager, spk_pipe) {
                Ok(ctrl) => {
                    tracing::info!("Speaker Denoise pipeline successfully started");
                    Some(Box::new(ctrl))
                }
                Err(err) => {
                    tracing::warn!("Failed to initialize Speaker Denoise pipeline: {}", err);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            running,
            metrics,
            controls,
            worker_handle: Some(worker_handle),
            _capture_stream: Some(capture_stream),
            _render_stream: render_stream,
            _ref_stream: ref_stream,
            _speaker_controller: speaker_controller,
        })
    }

    /// Starts secondary processing pipeline to clean incoming speaker audio (e.g. from Discord/Zoom)
    /// before sending it to headphones.
    pub fn start_speaker_denoise(
        device_manager: &DeviceManager,
        pipeline: AudioPipeline,
    ) -> Result<Self, String> {
        let running = Arc::new(AtomicBool::new(true));
        let metrics = Arc::new(AudioMetrics::new());

        let spk_in = device_manager.get_default_output_device()?;
        let spk_in_cfg = spk_in.default_output_config()
            .map_err(|e| format!("Failed to get speaker in config: {}", e))?;
        let in_channels = (spk_in_cfg.channels() as usize).max(1);

        let input_rb = HeapRb::<f32>::new(Self::RB_CAPACITY);
        let output_rb = HeapRb::<f32>::new(Self::RB_CAPACITY);
        #[allow(unused_mut)]
        let (mut in_prod, mut in_cons) = input_rb.split();
        #[allow(unused_mut)]
        let (mut out_prod, mut out_cons) = output_rb.split();

        let is_running_cap = Arc::clone(&running);
        let cap_stream = spk_in.build_input_stream(
            &spk_in_cfg.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if !is_running_cap.load(Ordering::Relaxed) {
                    return;
                }
                if in_channels > 1 {
                    for frame in data.chunks_exact(in_channels) {
                        let mono = frame.iter().sum::<f32>() / in_channels as f32;
                        let _ = in_prod.try_push(mono);
                    }
                } else {
                    for &s in data {
                        let _ = in_prod.try_push(s);
                    }
                }
            },
            |err| tracing::warn!("Speaker denoise capture error: {}", err),
            None,
        ).map_err(|e| format!("Failed to build speaker denoise capture stream: {}", e))?;

        cap_stream.play().map_err(|e| format!("Failed to start speaker denoise capture stream: {}", e))?;

        let is_running_render = Arc::clone(&running);
        let out_stream = spk_in.build_output_stream(
            &spk_in.default_output_config().map_err(|e| format!("Out cfg err: {}", e))?.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if !is_running_render.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    return;
                }
                if in_channels > 1 {
                    for frame in data.chunks_exact_mut(in_channels) {
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
            |err| tracing::warn!("Speaker denoise render error: {}", err),
            None,
        ).map_err(|e| format!("Failed to build speaker denoise render stream: {}", e))?;

        out_stream.play().map_err(|e| format!("Failed to start speaker denoise render: {}", e))?;

        let is_running_worker = Arc::clone(&running);
        let mut worker_pipeline = pipeline;
        let worker_handle = std::thread::Builder::new()
            .name("180hznc-speaker-dsp".to_string())
            .spawn(move || {
                let _mmcss = MmcssHandle::register_pro_audio();
                let mut in_chunk = vec![0.0f32; AudioPipeline::FRAME_SAMPLES];
                let mut out_chunk = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES + 10);

                while is_running_worker.load(Ordering::Relaxed) {
                    if in_cons.occupied_len() >= AudioPipeline::FRAME_SAMPLES {
                        for sample in in_chunk.iter_mut() {
                            *sample = in_cons.try_pop().unwrap_or(0.0);
                        }
                        let fill = out_prod.occupied_len();
                        let res = worker_pipeline.process_frame_internal(&in_chunk, &mut out_chunk, fill);
                        if res.is_ok() {
                            for &s in &out_chunk {
                                let _ = out_prod.try_push(s);
                            }
                        } else {
                            for &s in &in_chunk {
                                let _ = out_prod.try_push(s);
                            }
                        }
                    } else {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn speaker DSP thread: {}", e))?;

        Ok(Self {
            running,
            metrics,
            controls: AudioStreamControls::default(),
            worker_handle: Some(worker_handle),
            _capture_stream: Some(cap_stream),
            _render_stream: Some(out_stream),
            _ref_stream: None,
            _speaker_controller: None,
        })
    }

    pub fn metrics(&self) -> &AudioMetrics {
        &self.metrics
    }

    pub fn controls(&self) -> &AudioStreamControls {
        &self.controls
    }

    pub fn set_mic_volume(&self, volume: f32) {
        self.controls.mic_volume_bits.store(volume.to_bits(), Ordering::Relaxed);
    }

    pub fn set_push_to_talk(&self, enabled: bool) {
        self.controls.ptt_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn set_ptt_active(&self, active: bool) {
        self.controls.ptt_active.store(active, Ordering::Relaxed);
    }

    pub fn set_auto_sensitivity(&self, enabled: bool) {
        self.controls.auto_sensitivity.store(enabled, Ordering::Relaxed);
    }

    pub fn set_sensitivity_threshold(&self, threshold_dbfs: f32) {
        self.controls.sensitivity_threshold_dbfs.store(threshold_dbfs.to_bits(), Ordering::Relaxed);
    }

    pub fn set_aec_enabled(&self, enabled: bool) {
        self.controls.aec_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_aec_enabled(&self) -> bool {
        self.controls.aec_enabled.load(Ordering::Relaxed)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveform_snapshot_non_blocking_telemetry() {
        let metrics = AudioMetrics::new();
        let raw = vec![0.5f32; 480];
        let clean = vec![0.1f32; 480];

        metrics.update_waveform_snapshot(&raw, &clean);
        let (snap_raw, snap_clean) = metrics.get_waveform_snapshot();

        assert_eq!(snap_raw.len(), AudioMetrics::WAVEFORM_SNAPSHOT_LEN);
        assert_eq!(snap_clean.len(), AudioMetrics::WAVEFORM_SNAPSHOT_LEN);
        assert!((snap_raw[0] - 0.5).abs() < 1e-5);
        assert!((snap_clean[0] - 0.1).abs() < 1e-5);
    }
}
