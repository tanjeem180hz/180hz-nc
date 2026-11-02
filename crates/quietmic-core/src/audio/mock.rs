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
}
