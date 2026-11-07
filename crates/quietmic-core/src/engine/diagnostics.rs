use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub const NUM_DIAGNOSTIC_STAGES: usize = 8;

pub const STAGE_NAMES: [&str; NUM_DIAGNOSTIC_STAGES] = [
    "01_raw_input",
    "02_after_resample",
    "03_after_highpass",
    "04_after_aec",
    "05_after_denoise",
    "06_after_wet_dry_mix",
    "07_after_postprocess_limiter",
    "08_final_output",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageMetrics {
    pub stage_index: usize,
    pub stage_name: String,
    pub rms_db: f32,
    pub peak_db: f32,
    pub sample_rate: u32,
    pub channels: usize,
    pub frame_size: usize,
}

/// SPSC Tap Producer endpoints passed to AudioPipeline.
/// Real-time safe: non-blocking `try_push`, 0 allocations, 0 locks.
pub struct DiagnosticTapProducers {
    pub is_enabled: Arc<AtomicBool>,
    pub producers: [ringbuf::HeapProd<f32>; NUM_DIAGNOSTIC_STAGES],
    pub stage_rms_bits: [Arc<AtomicU64>; NUM_DIAGNOSTIC_STAGES],
    pub stage_peak_bits: [Arc<AtomicU64>; NUM_DIAGNOSTIC_STAGES],
    pub dropped_samples: Arc<AtomicU64>,
}

impl DiagnosticTapProducers {
    #[inline(always)]
    pub fn push_stage(&mut self, stage_idx: usize, samples: &[f32]) {
        if !self.is_enabled.load(Ordering::Relaxed) || stage_idx >= NUM_DIAGNOSTIC_STAGES {
            return;
        }

        let mut sum_sq = 0.0f32;
        let mut peak = 0.0f32;

        let prod = &mut self.producers[stage_idx];
        for &s in samples {
            let abs_s = s.abs();
            if abs_s > peak {
                peak = abs_s;
            }
            sum_sq += s * s;

            if prod.try_push(s).is_err() {
                self.dropped_samples.fetch_add(1, Ordering::Relaxed);
            }
        }

        let rms = (sum_sq / samples.len().max(1) as f32).sqrt();
        let rms_db = if rms > 1e-6 { 20.0 * rms.log10() } else { -120.0 };
        let peak_db = if peak > 1e-6 { 20.0 * peak.log10() } else { -120.0 };

        self.stage_rms_bits[stage_idx].store(rms_db.to_bits() as u64, Ordering::Relaxed);
        self.stage_peak_bits[stage_idx].store(peak_db.to_bits() as u64, Ordering::Relaxed);
    }
}

/// Generates a standardized 3-second test signal combining stationary fan hum (60/120 Hz),
/// synthetic speech vowels (300, 600, 1200 Hz) during 1.0-2.0s, and keyboard clicks at 0.5s, 1.5s, 2.5s.
pub fn generate_diagnostic_test_signal(duration_secs: f32) -> Vec<f32> {
    let total_samples = (48000.0 * duration_secs) as usize;
    let mut signal = vec![0.0f32; total_samples];
    let mut rng = 123456789u32;

    for (i, sample) in signal.iter_mut().enumerate() {
        let t = i as f32 / 48000.0;
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise_rand = (rng as f32 / u32::MAX as f32 - 0.5) * 0.02;

        let fan = 0.03 * (2.0 * std::f32::consts::PI * 60.0 * t).sin()
            + 0.015 * (2.0 * std::f32::consts::PI * 120.0 * t).sin()
            + noise_rand;

        let speech = if (1.0..2.0).contains(&t) {
            0.20 * (2.0 * std::f32::consts::PI * 300.0 * t).sin()
                + 0.15 * (2.0 * std::f32::consts::PI * 600.0 * t).sin()
                + 0.10 * (2.0 * std::f32::consts::PI * 1200.0 * t).sin()
        } else {
            0.0
        };

        let click = if (t - 0.5).abs() < 0.005 || (t - 1.5).abs() < 0.005 || (t - 2.5).abs() < 0.005 {
            0.25 * (2.0 * std::f32::consts::PI * 3500.0 * t).sin()
        } else {
            0.0
        };

        *sample = fan + speech + click;
    }

    signal
}

/// Background Disk Dumper for Diagnostic WAV files.
/// Runs in its own thread to ensure audio callbacks never touch disk I/O.
pub struct DiagnosticController {
    is_enabled: Arc<AtomicBool>,
    output_dir: PathBuf,
    stage_rms_bits: [Arc<AtomicU64>; NUM_DIAGNOSTIC_STAGES],
    stage_peak_bits: [Arc<AtomicU64>; NUM_DIAGNOSTIC_STAGES],
    dropped_samples: Arc<AtomicU64>,
    consumers: Option<Vec<ringbuf::HeapCons<f32>>>,
    writer_handle: Option<JoinHandle<()>>,
    stop_signal: Arc<AtomicBool>,
}

impl DiagnosticController {
    pub const BUFFER_CAPACITY: usize = 48000 * 2; // 2 seconds of audio headroom

    pub fn new(output_dir: PathBuf) -> (Self, DiagnosticTapProducers) {
        let is_enabled = Arc::new(AtomicBool::new(false));
        let stop_signal = Arc::new(AtomicBool::new(false));
        let dropped_samples = Arc::new(AtomicU64::new(0));

        let mut prods = Vec::with_capacity(NUM_DIAGNOSTIC_STAGES);
        let mut conss = Vec::with_capacity(NUM_DIAGNOSTIC_STAGES);

        let mut rms_arcs: Vec<Arc<AtomicU64>> = Vec::with_capacity(NUM_DIAGNOSTIC_STAGES);
        let mut peak_arcs: Vec<Arc<AtomicU64>> = Vec::with_capacity(NUM_DIAGNOSTIC_STAGES);

        for _ in 0..NUM_DIAGNOSTIC_STAGES {
            let rb = HeapRb::<f32>::new(Self::BUFFER_CAPACITY);
            let (p, c) = rb.split();
            prods.push(p);
            conss.push(c);
            rms_arcs.push(Arc::new(AtomicU64::new((-100.0f32).to_bits() as u64)));
            peak_arcs.push(Arc::new(AtomicU64::new((-100.0f32).to_bits() as u64)));
        }

        let stage_rms_bits: [Arc<AtomicU64>; NUM_DIAGNOSTIC_STAGES] = rms_arcs.clone().try_into().unwrap();
        let stage_peak_bits: [Arc<AtomicU64>; NUM_DIAGNOSTIC_STAGES] = peak_arcs.clone().try_into().unwrap();

        let tap_producers = DiagnosticTapProducers {
            is_enabled: Arc::clone(&is_enabled),
            producers: prods.try_into().unwrap_or_else(|_| panic!("Stage count mismatch")),
            stage_rms_bits: rms_arcs.try_into().unwrap(),
            stage_peak_bits: peak_arcs.try_into().unwrap(),
            dropped_samples: Arc::clone(&dropped_samples),
        };

        let controller = Self {
            is_enabled,
            output_dir,
            stage_rms_bits,
            stage_peak_bits,
            dropped_samples,
            consumers: Some(conss),
            writer_handle: None,
            stop_signal,
        };

        (controller, tap_producers)
    }

    /// Starts writing diagnostic WAV files to disk from the background thread.
    pub fn start(&mut self) -> Result<(), String> {
        self.stop();

        std::fs::create_dir_all(&self.output_dir).map_err(|e| e.to_string())?;
        self.stop_signal.store(false, Ordering::Relaxed);
        self.is_enabled.store(true, Ordering::Relaxed);

        let out_dir = self.output_dir.clone();
        let stop_sig = Arc::clone(&self.stop_signal);

        let consumers = self.consumers.take().ok_or_else(|| {
            "Diagnostic consumers already started or detached".to_string()
        })?;

        let handle = thread::Builder::new()
            .name("quietmic-diagnostic-writer".to_string())
            .spawn(move || {
                let spec = hound::WavSpec {
                    channels: 1,
                    sample_rate: 48000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                };

                let mut writers: Vec<Option<hound::WavWriter<BufWriter<File>>>> = Vec::new();
                for &name in &STAGE_NAMES {
                    let path = out_dir.join(format!("{}.wav", name));
                    match hound::WavWriter::create(path, spec) {
                        Ok(w) => writers.push(Some(w)),
                        Err(e) => {
                            tracing::error!("Failed to create diagnostic wav for {}: {}", name, e);
                            writers.push(None);
                        }
                    }
                }

                let mut consumers = consumers;

                while !stop_sig.load(Ordering::Relaxed) {
                    let mut had_samples = false;
                    for (idx, cons) in consumers.iter_mut().enumerate() {
                        if let Some(ref mut writer) = writers[idx] {
                            while let Some(sample) = cons.try_pop() {
                                had_samples = true;
                                let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                                let _ = writer.write_sample(sample_i16);
                            }
                        }
                    }

                    if !had_samples {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                }

                // Final drain and flush
                for (idx, cons) in consumers.iter_mut().enumerate() {
                    if let Some(mut writer) = writers[idx].take() {
                        while let Some(sample) = cons.try_pop() {
                            let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                            let _ = writer.write_sample(sample_i16);
                        }
                        let _ = writer.finalize();
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        self.writer_handle = Some(handle);
        Ok(())
    }

    /// Stops diagnostic recording and flushes all WAV files.
    pub fn stop(&mut self) {
        self.is_enabled.store(false, Ordering::Relaxed);
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(handle) = self.writer_handle.take() {
            let _ = handle.join();
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.is_enabled.load(Ordering::Relaxed)
    }

    pub fn get_metrics(&self) -> Vec<StageMetrics> {
        let mut list = Vec::with_capacity(NUM_DIAGNOSTIC_STAGES);
        for (idx, &name) in STAGE_NAMES.iter().enumerate().take(NUM_DIAGNOSTIC_STAGES) {
            let rms_u64 = self.stage_rms_bits[idx].load(Ordering::Relaxed);
            let peak_u64 = self.stage_peak_bits[idx].load(Ordering::Relaxed);
            let rms_db = f32::from_bits(rms_u64 as u32);
            let peak_db = f32::from_bits(peak_u64 as u32);

            list.push(StageMetrics {
                stage_index: idx,
                stage_name: name.to_string(),
                rms_db,
                peak_db,
                sample_rate: 48000,
                channels: 1,
                frame_size: 480,
            });
        }
        list
    }

    pub fn dropped_samples(&self) -> u64 {
        self.dropped_samples.load(Ordering::Relaxed)
    }
}

/// Runs a full diagnostic audio sweep offline through all 8 stages, saving WAV files to `output_dir`
/// and returning per-stage metrics.
pub fn run_diagnostic_sweep(
    pipeline: &mut crate::engine::pipeline::AudioPipeline,
    input_samples: &[f32],
    output_dir: &std::path::Path,
) -> Result<Vec<StageMetrics>, String> {
    let (mut controller, tap) = DiagnosticController::new(output_dir.to_path_buf());
    pipeline.set_diagnostic_tap(Some(tap));

    controller.start()?;

    let mut out_frame = Vec::with_capacity(500);
    let frame_size = 480;
    let mut offset = 0;

    while offset + frame_size <= input_samples.len() {
        let chunk = &input_samples[offset..offset + frame_size];
        pipeline.process_frame_internal(chunk, &mut out_frame, 960)?;
        offset += frame_size;
    }

    // Flush and finish
    controller.stop();
    pipeline.set_diagnostic_tap(None);

    Ok(controller.get_metrics())
}

impl Drop for DiagnosticController {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::pipeline::{AudioPipeline, EngineMode};

    #[test]
    fn test_diagnostic_controller_dumps_all_8_stages() {
        let dump_dir = std::env::temp_dir().join(format!(
            "quietmic_diag_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let (mut controller, tap) = DiagnosticController::new(dump_dir.clone());

        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_mode(EngineMode::Light);
        pipeline.set_diagnostic_tap(Some(tap));

        assert!(controller.start().is_ok());
        assert!(controller.is_enabled());

        let test_frame: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.2).collect();
        let mut out_frame = Vec::with_capacity(500);

        for _ in 0..10 {
            let res = pipeline.process_frame_internal(&test_frame, &mut out_frame, 960);
            assert!(res.is_ok());
        }

        let metrics = controller.get_metrics();
        assert_eq!(metrics.len(), NUM_DIAGNOSTIC_STAGES);
        for m in &metrics {
            assert!(
                m.rms_db > -120.0,
                "Stage {} has silent RMS: {}",
                m.stage_name,
                m.rms_db
            );
        }

        controller.stop();
        assert!(!controller.is_enabled());

        // Verify that all 8 WAV files exist and are valid hound WAV files
        for &name in &STAGE_NAMES {
            let wav_path = dump_dir.join(format!("{}.wav", name));
            assert!(wav_path.exists(), "WAV file does not exist: {:?}", wav_path);
            let reader = hound::WavReader::open(&wav_path).expect("Failed to read diagnostic WAV");
            assert_eq!(reader.spec().sample_rate, 48000);
            assert_eq!(reader.spec().channels, 1);
            assert_eq!(reader.duration(), 4800); // 10 frames of 480 = 4800 samples
        }

        let _ = std::fs::remove_dir_all(&dump_dir);
    }

    #[test]
    fn test_diagnostic_sweep_noisy_file() {
        let dump_dir = std::path::PathBuf::from("target/diagnostics_run");
        let _ = std::fs::remove_dir_all(&dump_dir);
        std::fs::create_dir_all(&dump_dir).unwrap();

        let total_samples = 48000 * 3;
        let mut test_audio = vec![0.0f32; total_samples];
        let mut speech_clean = vec![0.0f32; total_samples];

        let mut rng = 123456789u32;
        for i in 0..total_samples {
            let t = i as f32 / 48000.0;
            // Linear congruential generator for pink/white noise in [-0.02, 0.02]
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise_rand = (rng as f32 / u32::MAX as f32 - 0.5) * 0.02;

            // Fan hum (60 Hz + 120 Hz)
            let fan = 0.03 * (2.0 * std::f32::consts::PI * 60.0 * t).sin()
                    + 0.015 * (2.0 * std::f32::consts::PI * 120.0 * t).sin()
                    + noise_rand;

            // Speech only in middle second (t = 1.0..2.0)
            let speech = if (1.0..2.0).contains(&t) {
                0.20 * (2.0 * std::f32::consts::PI * 300.0 * t).sin()
                    + 0.15 * (2.0 * std::f32::consts::PI * 600.0 * t).sin()
                    + 0.10 * (2.0 * std::f32::consts::PI * 1200.0 * t).sin()
            } else {
                0.0
            };

            speech_clean[i] = speech;

            // Keyboard clicks at 0.5s, 1.5s, 2.5s
            let click = if (t - 0.5).abs() < 0.005 || (t - 1.5).abs() < 0.005 || (t - 2.5).abs() < 0.005 {
                0.25 * (2.0 * std::f32::consts::PI * 3500.0 * t).sin()
            } else {
                0.0
            };

            test_audio[i] = fan + speech + click;
        }

        for &mode in &[EngineMode::Standard, EngineMode::Light] {
            println!("\n========================================================");
            println!("   DIAGNOSTIC STAGE EVALUATION: {:?}", mode);
            println!("========================================================");

            let mode_str = match mode {
                EngineMode::Standard => "standard",
                EngineMode::Light => "light",
                _ => "voice_isolation",
            };
            let mode_dir = dump_dir.join(mode_str);

            let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
            pipeline.set_mode(mode);
            pipeline.set_strength(1.0);

            let _ = run_diagnostic_sweep(&mut pipeline, &test_audio, &mode_dir).unwrap();

            // Analyze each dumped stage WAV file across Speech vs Noise segments
            println!("{:<30} | {:<12} | {:<12} | {:<10}", "Stage", "Noise RMS", "Speech RMS", "SNR Diff");
            println!("{:-<72}", "");

            for &name in &STAGE_NAMES {
                let wav_path = mode_dir.join(format!("{}.wav", name));
                let mut reader = hound::WavReader::open(&wav_path).unwrap();
                let samples: Vec<f32> = reader.samples::<i16>().map(|s| s.unwrap() as f32 / i16::MAX as f32).collect();

                // Noise-only region: 0.1s to 0.9s (samples 4800 to 43200)
                let noise_samples = &samples[4800..43200];
                let noise_sum_sq: f32 = noise_samples.iter().map(|&x| x * x).sum();
                let noise_rms = (noise_sum_sq / noise_samples.len() as f32).sqrt();
                let noise_rms_db = if noise_rms > 1e-6 { 20.0 * noise_rms.log10() } else { -120.0 };

                // Speech region: 1.1s to 1.9s (samples 52800 to 91200)
                let speech_samples = &samples[52800..91200];
                let speech_sum_sq: f32 = speech_samples.iter().map(|&x| x * x).sum();
                let speech_rms = (speech_sum_sq / speech_samples.len() as f32).sqrt();
                let speech_rms_db = if speech_rms > 1e-6 { 20.0 * speech_rms.log10() } else { -120.0 };

                let snr_diff = speech_rms_db - noise_rms_db;

                println!("{:<30} | {:>9.1} dB | {:>9.1} dB | {:>8.1} dB", name, noise_rms_db, speech_rms_db, snr_diff);
            }
        }
    }
}
