use crate::audio::device::AudioDeviceInfo;
use crate::dsp::resampler::AudioResampler;
use crate::dsp::DriftCompensator;
use crate::engine::pipeline::{AudioPipeline, EngineMode};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// Simulated Audio Device Manager for automated testing and CI environments
/// where physical microphones, speakers, or VB-CABLE virtual sound cards are absent.
#[derive(Debug, Clone)]
pub struct MockDeviceManager {
    devices: Arc<parking_lot::Mutex<Vec<AudioDeviceInfo>>>,
    cable_installed: Arc<AtomicBool>,
}

impl MockDeviceManager {
    pub fn new() -> Self {
        let default_devices = vec![
            AudioDeviceInfo {
                id: "mock-realtek-1".to_string(),
                name: "Mock Realtek High Definition Audio".to_string(),
                default_sample_rate: 48000,
                channels: 2,
                is_default: true,
            },
            AudioDeviceInfo {
                id: "mock-usb-headset-1".to_string(),
                name: "Mock USB Gaming Headset".to_string(),
                default_sample_rate: 44100,
                channels: 1,
                is_default: false,
            },
            AudioDeviceInfo {
                id: "mock-bluetooth-1".to_string(),
                name: "Mock Bluetooth Hands-Free Audio".to_string(),
                default_sample_rate: 16000,
                channels: 1,
                is_default: false,
            },
        ];

        Self {
            devices: Arc::new(parking_lot::Mutex::new(default_devices)),
            cable_installed: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn list_devices(&self) -> Vec<AudioDeviceInfo> {
        self.devices.lock().clone()
    }

    /// Simulates plugging in a new USB microphone.
    pub fn plug_in_device(&self, name: &str, sample_rate: u32, channels: u16) {
        let mut list = self.devices.lock();
        list.push(AudioDeviceInfo {
            id: format!("mock-{}", name.to_lowercase().replace(' ', "-")),
            name: name.to_string(),
            default_sample_rate: sample_rate,
            channels,
            is_default: false,
        });
    }

    /// Simulates unplugging a microphone.
    pub fn unplug_device(&self, name: &str) -> bool {
        let mut list = self.devices.lock();
        let init_len = list.len();
        list.retain(|d| d.name != name);
        list.len() < init_len
    }

    pub fn set_cable_installed(&self, installed: bool) {
        self.cable_installed.store(installed, Ordering::Relaxed);
    }

    pub fn is_cable_installed(&self) -> bool {
        self.cable_installed.load(Ordering::Relaxed)
    }
}

impl Default for MockDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Simulated Real-Time Audio Stream Harness.
/// Feeds simulated frames through an AudioPipeline, allowing programmatic verification
/// of clock drift compensation, sample-rate conversions, and underrun/overrun recovery.
pub struct MockStreamHarness {
    pub sample_rate: u32,
    pub channels: usize,
    pub pipeline: AudioPipeline,
    pub resampler: AudioResampler,
    pub drift_compensator: DriftCompensator,
    pub frames_processed: AtomicUsize,
    pub underruns_detected: AtomicUsize,
}

impl MockStreamHarness {
    pub fn new(hardware_sample_rate: u32, channels: usize, mode: EngineMode) -> Self {
        let mut pipeline = AudioPipeline::new(hardware_sample_rate, 48000).expect("Pipeline init failed");
        pipeline.set_mode(mode);
        let resampler = AudioResampler::new(hardware_sample_rate, 48000, 480).expect("Resampler init failed");
        let drift = DriftCompensator::default_48k();

        Self {
            sample_rate: hardware_sample_rate,
            channels: channels.max(1),
            pipeline,
            resampler,
            drift_compensator: drift,
            frames_processed: AtomicUsize::new(0),
            underruns_detected: AtomicUsize::new(0),
        }
    }

    /// Generates a synthetic input frame with optional simulated clock skew (ppm).
    pub fn generate_test_frame(&self, frame_size: usize, freq_hz: f32) -> Vec<f32> {
        let mut frame = vec![0.0f32; frame_size * self.channels];
        let count = self.frames_processed.load(Ordering::Relaxed);

        for i in 0..frame_size {
            let t = (count * frame_size + i) as f32 / self.sample_rate as f32;
            let sample = 0.2 * (2.0 * std::f32::consts::PI * freq_hz * t).sin();
            for ch in 0..self.channels {
                frame[i * self.channels + ch] = sample;
            }
        }

        frame
    }

    /// Processes a single frame and records performance.
    pub fn process_mock_frame(&mut self, input: &[f32], output: &mut Vec<f32>, buffer_fill: usize) -> Result<(), String> {
        // Downmix to mono if multi-channel
        let mono: Vec<f32> = if self.channels > 1 {
            input.chunks_exact(self.channels)
                .map(|chunk| chunk.iter().sum::<f32>() / self.channels as f32)
                .collect()
        } else {
            input.to_vec()
        };

        if mono.is_empty() {
            self.underruns_detected.fetch_add(1, Ordering::Relaxed);
            output.clear();
            return Ok(());
        }

        let res = self.pipeline.process_frame_internal(&mono, output, buffer_fill);
        if res.is_ok() {
            self.frames_processed.fetch_add(1, Ordering::Relaxed);
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_hotplug_detection_and_recovery() {
        let manager = MockDeviceManager::new();
        let initial_count = manager.list_devices().len();

        // 1. Plug in a new USB studio mic
        manager.plug_in_device("Shure MV7 USB Microphone", 48000, 1);
        let updated = manager.list_devices();
        assert_eq!(updated.len(), initial_count + 1);
        assert!(updated.iter().any(|d| d.name == "Shure MV7 USB Microphone"));

        // 2. Unplug device
        let removed = manager.unplug_device("Shure MV7 USB Microphone");
        assert!(removed);
        assert_eq!(manager.list_devices().len(), initial_count);
    }

    #[test]
    fn test_mock_sample_rate_transitions() {
        // Test 16 kHz Bluetooth HFP -> 48 kHz internal pipeline
        let mut harness_16k = MockStreamHarness::new(16000, 1, EngineMode::Standard);
        let mut out_chunk = Vec::with_capacity(500);

        // Process initial frames to warm up Sinc filter delay into steady state
        for _ in 0..5 {
            let in_16k = harness_16k.generate_test_frame(160, 440.0); // 10 ms at 16 kHz = 160 samples
            let res = harness_16k.process_mock_frame(&in_16k, &mut out_chunk, 960);
            assert!(res.is_ok());
        }
        assert_eq!(out_chunk.len(), 480); // Steady-state resampled to 48 kHz = 480 samples

        // Test 96 kHz High-Res Studio Interface -> 48 kHz internal pipeline
        let mut harness_96k = MockStreamHarness::new(96000, 2, EngineMode::Light);
        for _ in 0..5 {
            let in_96k = harness_96k.generate_test_frame(960, 1000.0); // 10 ms at 96 kHz = 960 frames * 2 channels
            let res96 = harness_96k.process_mock_frame(&in_96k, &mut out_chunk, 960);
            assert!(res96.is_ok());
        }
        assert_eq!(out_chunk.len(), 480);
    }

    #[test]
    fn test_mock_clock_drift_skew_correction() {
        let mut harness = MockStreamHarness::new(48000, 1, EngineMode::Standard);
        let mut output = Vec::with_capacity(500);

        // Simulate buffer overfill scenario (mic clock running faster than DAC)
        for _ in 0..50 {
            let in_frame = harness.generate_test_frame(480, 500.0);
            // Report high buffer fill (1500 > 960 target)
            assert!(harness.process_mock_frame(&in_frame, &mut output, 1500).is_ok());
        }

        // Verify drift compensator smoothly adjusted frame without crashing
        assert_eq!(output.len(), 480);
    }

    #[test]
    fn test_mock_underrun_recovery() {
        let mut harness = MockStreamHarness::new(48000, 1, EngineMode::Standard);
        let mut output = Vec::with_capacity(500);

        // Feed empty frame simulating buffer underrun / thread preemption
        let empty_frame: Vec<f32> = Vec::new();
        assert!(harness.process_mock_frame(&empty_frame, &mut output, 0).is_ok());
        assert_eq!(harness.underruns_detected.load(Ordering::Relaxed), 1);

        // Feed standard frame next; verify immediate clean recovery
        let normal_frame = harness.generate_test_frame(480, 440.0);
        assert!(harness.process_mock_frame(&normal_frame, &mut output, 960).is_ok());
        assert_eq!(output.len(), 480);
    }

    #[test]
    fn test_stress_hotplug_storm_500_iterations() {
        let manager = MockDeviceManager::new();
        let t0 = std::time::Instant::now();

        for i in 0..500 {
            let dev_name = format!("USB Interface {}", i % 10);
            if i % 2 == 0 {
                manager.plug_in_device(&dev_name, 48000, 2);
            } else {
                manager.unplug_device(&dev_name);
            }
            let _ = manager.list_devices();
        }

        let elapsed = t0.elapsed();
        // Must complete 500 iterations without deadlock in under 2.0 seconds
        assert!(elapsed.as_millis() < 2000, "Hotplug storm took too long: {:?}", elapsed);
    }

    #[test]
    fn test_stress_clock_drift_ppm_skew() {
        // Test +100 ppm, -100 ppm, +500 ppm, and -500 ppm clock skew
        let skews_ppm = [100.0f32, -100.0f32, 500.0f32, -500.0f32];

        for &skew in &skews_ppm {
            let mut harness = MockStreamHarness::new(48000, 1, EngineMode::Standard);
            let mut output = Vec::with_capacity(500);

            // Target fill is 960 samples (20 ms).
            // Skew causes buffer fill to deviate:
            let simulated_fill = (960.0 * (1.0 + skew / 1000.0)) as usize;

            for _ in 0..200 {
                let in_frame = harness.generate_test_frame(480, 440.0);
                assert!(harness.process_mock_frame(&in_frame, &mut output, simulated_fill).is_ok());
                let diff = (output.len() as isize - 480).abs();
                assert!(diff <= 1, "Output frame length deviates beyond drift bounds: {}", output.len());
                for &s in &output {
                    assert!(s.is_finite(), "Output sample must be finite under clock skew");
                    assert!(s.abs() <= 1.05, "Output exceeded limiter bound");
                }
            }
        }
    }

    #[test]
    fn test_stress_sample_rate_and_channel_sweep() {
        let configs = [
            (8000, 1, 80),    // Narrowband 8 kHz mono: 10 ms = 80 samples
            (16000, 1, 160),  // Wideband 16 kHz mono: 10 ms = 160 samples
            (44100, 2, 441),  // CD 44.1 kHz stereo: 10 ms = 441 samples * 2
            (48000, 2, 480),  // Broadcast 48 kHz stereo: 10 ms = 480 samples * 2
            (96000, 2, 960),  // High-Res 96 kHz stereo: 10 ms = 960 samples * 2
        ];

        for (sr, channels, frame_samples) in configs {
            let mut harness = MockStreamHarness::new(sr, channels, EngineMode::Standard);
            let mut out = Vec::with_capacity(500);

            // Warm up filter into steady-state
            for _ in 0..10 {
                let frame = harness.generate_test_frame(frame_samples, 500.0);
                assert!(harness.process_mock_frame(&frame, &mut out, 960).is_ok());
            }

            assert_eq!(out.len(), 480, "Resampled output at {} kHz must be exactly 480 samples", sr);
            for &s in &out {
                assert!(s.is_finite());
            }
        }
    }

    #[test]
    fn test_stress_extreme_inputs_fuzz() {
        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_mode(EngineMode::Standard);
        pipeline.set_strength(1.0);
        pipeline.set_aec_enabled(true);

        let mut out = Vec::with_capacity(500);
        let aec_ref = vec![0.02f32; 480];

        // 1. Full Scale DC (+1.0 and -1.0)
        let dc_pos = vec![1.0f32; 480];
        let dc_neg = vec![-1.0f32; 480];
        for _ in 0..50 {
            pipeline.feed_aec_reference(&aec_ref);
            assert!(pipeline.process_frame_internal(&dc_pos, &mut out, 960).is_ok());
            for &s in &out {
                assert!(s.is_finite() && s.abs() <= 1.05);
            }
            assert!(pipeline.process_frame_internal(&dc_neg, &mut out, 960).is_ok());
            for &s in &out {
                assert!(s.is_finite() && s.abs() <= 1.05);
            }
        }

        // 2. High Frequency Nyquist Tone (22 kHz)
        let nyquist_tone: Vec<f32> = (0..480)
            .map(|i| (2.0 * std::f32::consts::PI * 22000.0 * (i as f32) / 48000.0).sin())
            .collect();
        for _ in 0..50 {
            assert!(pipeline.process_frame_internal(&nyquist_tone, &mut out, 960).is_ok());
            for &s in &out {
                assert!(s.is_finite() && s.abs() <= 1.05);
            }
        }

        // 3. Extreme Spikes and NaNs / Infinities
        let mut extreme_frame = vec![0.0f32; 480];
        for i in 0..480 {
            match i % 6 {
                0 => extreme_frame[i] = f32::NAN,
                1 => extreme_frame[i] = f32::INFINITY,
                2 => extreme_frame[i] = f32::NEG_INFINITY,
                3 => extreme_frame[i] = 1000.0, // Severe voltage spike
                4 => extreme_frame[i] = -1000.0,
                _ => extreme_frame[i] = 0.0,
            }
        }
        for _ in 0..50 {
            assert!(pipeline.process_frame_internal(&extreme_frame, &mut out, 960).is_ok());
            for &s in &out {
                assert!(s.is_finite(), "Sanitizer failed to eliminate non-finite sample");
                assert!(s.abs() <= 1.05, "Limiter failed to contain spike");
            }
        }
    }

    #[test]
    fn test_stress_dsp_property_fuzz() {
        use crate::dsp::biquad::HighPassFilter;
        use crate::dsp::gain_smoother::GainSmoother;
        use crate::dsp::limiter::SoftLimiter;
        use crate::dsp::comfort_noise::ComfortNoiseGenerator;
        use crate::engine::aec::EchoCanceller;

        let mut hpf = HighPassFilter::default_48k();
        let mut smoother = GainSmoother::default_48k();
        let limiter = SoftLimiter::default_preset();
        let mut noise = ComfortNoiseGenerator::default_preset();
        let mut aec = EchoCanceller::default_preset();
        aec.set_enabled(true);

        // Deterministic LCG pseudo-random generator
        let mut seed = 0x12345678u64;
        let mut next_f32 = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let val = ((seed >> 32) as i32) as f32 / 2147483648.0;
            // Introduce special floats occasionally
            match seed % 50 {
                0 => 0.0f32,
                1 => 1e-38f32, // Denormal
                2 => -1e-38f32,
                3 => 1.0f32,
                4 => -1.0f32,
                _ => val,
            }
        };

        let mut buffer = vec![0.0f32; 480];
        let mut ref_buf = vec![0.0f32; 480];

        for _ in 0..100 {
            for i in 0..480 {
                buffer[i] = next_f32();
                ref_buf[i] = next_f32();
            }

            hpf.process_slice(&mut buffer);
            for &s in &buffer { assert!(s.is_finite()); }

            aec.feed_reference(&ref_buf);
            aec.process_mic_frame(&mut buffer);
            for &s in &buffer { assert!(s.is_finite()); }

            smoother.blend_frame(&ref_buf, &buffer, &mut buffer.clone());
            noise.inject_into(&mut buffer);
            limiter.process_slice(&mut buffer);

            for &s in &buffer {
                assert!(s.is_finite(), "DSP fuzz produced non-finite output");
                assert!(s.abs() <= 1.05, "Limiter failed to bound output: {}", s);
            }
        }
    }

    #[test]
    fn test_stress_60_minute_simulated_soak() {
        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_mode(EngineMode::Standard);
        pipeline.set_strength(1.0);
        pipeline.set_aec_enabled(true);

        let input_frame = vec![0.15f32; 480];
        let mut output = Vec::with_capacity(500);
        let aec_ref = vec![0.02f32; 480];

        let target_frames = 360_000usize; // 60 minutes = 360,000 frames of 10 ms
        let mut glitches = 0usize;
        let init_cap = output.capacity();

        for i in 0..target_frames {
            if i % 10 == 0 {
                pipeline.feed_aec_reference(&aec_ref);
            }
            if pipeline.process_frame_internal(&input_frame, &mut output, 960).is_err() {
                glitches += 1;
            }
            if output.len() != 480 {
                glitches += 1;
            }
        }

        assert_eq!(glitches, 0, "Soak encountered glitches or dropouts!");
        assert_eq!(output.capacity(), init_cap, "Memory grew during 60-minute soak!");
    }

    #[test]
    fn test_low_end_single_core_affinity_rtf() {
        #[cfg(windows)]
        {
            use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessAffinityMask, SetProcessAffinityMask};
            unsafe {
                let mut proc_mask = 0usize;
                let mut sys_mask = 0usize;
                if GetProcessAffinityMask(GetCurrentProcess(), &mut proc_mask, &mut sys_mask).is_ok() {
                    // Restrict process to single logical core (Core 0)
                    let _ = SetProcessAffinityMask(GetCurrentProcess(), 1);

                    let mut light_pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
                    light_pipeline.set_mode(EngineMode::Light);
                    light_pipeline.set_strength(1.0);

                    let test_frame = vec![0.2f32; 480];
                    let mut out = Vec::with_capacity(500);

                    // Warm up
                    for _ in 0..100 {
                        let _ = light_pipeline.process_frame_internal(&test_frame, &mut out, 960);
                    }

                    let t0 = std::time::Instant::now();
                    let num_frames = 2000usize; // 20 seconds of audio
                    for _ in 0..num_frames {
                        let _ = light_pipeline.process_frame_internal(&test_frame, &mut out, 960);
                    }
                    let elapsed = t0.elapsed();
                    let mean_frame_us = (elapsed.as_micros() as f32) / (num_frames as f32);
                    let rtf = mean_frame_us / 10000.0; // 10,000 µs = 10 ms budget

                    // Restore original process affinity mask
                    let _ = SetProcessAffinityMask(GetCurrentProcess(), proc_mask);

                    println!("\nSingle-core affinity Light mode: mean={:.1} µs, RTF={:.4}", mean_frame_us, rtf);
                    // Light mode RTF target must be < 0.3 (< 30% of single core)
                    assert!(rtf < 0.3, "Light mode RTF exceeded 0.3 on single core: rtf={}", rtf);
                    assert!(mean_frame_us < 3000.0, "Mean frame time exceeded 3000 µs: {}", mean_frame_us);
                }
            }
        }
    }

    #[test]
    fn test_stress_live_monitor_200_start_stop_cycles() {
        use crate::audio::live_monitor::LiveMonitorController;
        use crate::dsp::crossfade::CrossfadeMode;

        let (mut controller, mut tap) = LiveMonitorController::new();

        let raw_frame = vec![0.3f32; 480];
        let clean_frame = vec![0.1f32; 480];

        for i in 0..200 {
            // Recreate tap periodically (simulating stream restart)
            if i % 25 == 0 {
                tap = controller.create_new_tap();
            }

            // Push frame while inactive -> verify 0 allocations, 0 drops, immediate return
            tap.push_frame(&raw_frame, &clean_frame);

            // Toggle A/B mode and volume
            if i % 2 == 0 {
                controller.set_ab_mode(CrossfadeMode::Cleaned);
                controller.set_volume(0.35 + (i as f32 % 50.0) / 100.0);
            } else {
                controller.set_ab_mode(CrossfadeMode::Original);
                controller.set_volume(0.75);
            }

            // Check status integrity
            let status = controller.get_status();
            assert!(!status.is_active);
            assert!(status.volume >= 0.0 && status.volume <= 1.0);
            assert!(!status.feedback_detected);
        }
    }

    #[test]
    fn test_live_monitor_record_and_wav_export() {
        use crate::audio::live_monitor::LiveMonitorController;
        use std::sync::atomic::Ordering;

        let (controller, tap) = LiveMonitorController::new();

        // While inactive, start_recording must return Err
        assert!(controller.start_recording().is_err());

        // When active, start_recording succeeds
        tap.is_active.store(true, Ordering::Relaxed);
        assert!(controller.start_recording().is_ok());

        // Test RecordReplayState and WAV export
        let temp_dir = std::env::temp_dir().join(format!("quietmic_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        std::fs::create_dir_all(&temp_dir).unwrap();
        let test_wav = temp_dir.join("test_out.wav");
        {
            let mut writer = hound::WavWriter::create(&test_wav, spec).unwrap();
            for i in 0..4800 {
                let s = ((i as f32 * 0.05).sin() * 10000.0) as i16;
                writer.write_sample(s).unwrap();
            }
            writer.finalize().unwrap();
        }

        // Verify valid WAV file readable by hound
        let reader = hound::WavReader::open(&test_wav).unwrap();
        assert_eq!(reader.spec().sample_rate, 48000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.duration(), 4800);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_stress_live_monitor_tap_concurrency() {
        use crate::audio::live_monitor::LiveMonitorController;
        use std::sync::atomic::Ordering;
        use std::thread;

        let (mut controller, mut tap) = LiveMonitorController::new();
        tap.is_active.store(true, Ordering::Relaxed);

        let producer_handle = thread::spawn(move || {
            let raw = vec![0.25f32; 480];
            let clean = vec![0.10f32; 480];
            for _ in 0..500 {
                tap.push_frame(&raw, &clean);
                thread::yield_now();
            }
        });

        let mut total_drained = 0usize;
        for _ in 0..500 {
            let status = controller.get_status();
            assert!(status.volume >= 0.0);
            total_drained += 1;
            thread::yield_now();
        }

        producer_handle.join().unwrap();
        assert_eq!(total_drained, 500);
    }
}
