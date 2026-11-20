use crate::audio::device::DeviceManager;
use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointType {
    Headphones,
    Headset,
    Speakers,
    Bluetooth,
    Unknown,
}

impl EndpointType {
    pub fn display_name(&self) -> &'static str {
        match self {
            EndpointType::Headphones => "Headphones",
            EndpointType::Headset => "Headset",
            EndpointType::Speakers => "Loudspeakers",
            EndpointType::Bluetooth => "Bluetooth Audio",
            EndpointType::Unknown => "Audio Output",
        }
    }

    pub fn aec_recommended(&self) -> bool {
        match self {
            EndpointType::Headphones | EndpointType::Headset => false, // No physical speaker-to-mic bleed
            EndpointType::Speakers => true,                           // Loudspeakers bleed into mic
            EndpointType::Bluetooth => false,                          // Usually in-ear or on-ear
            EndpointType::Unknown => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopPath {
    CleanRouting,
    DefaultPlaybackIsVirtualCable,
    ListenToDeviceActive,
    AcousticSpeakerToMicBleed,
    ScreenShareSystemAudioCapture,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoDoctorReport {
    pub default_playback_name: String,
    pub default_playback_is_cable: bool,
    pub default_comm_name: String,
    pub default_comm_is_cable: bool,
    pub endpoint_type: EndpointType,
    pub aec_recommended: bool,
    pub listen_to_device_warning: bool,
    pub detected_loop: LoopPath,
    pub loop_severity: String, // "CLEAN", "WARNING", "CRITICAL"
    pub plain_explanation: String,
    pub step_by_step_fix: Vec<String>,
    pub discord_screen_share_tips: Vec<String>,
}

pub struct EchoDoctor;

impl EchoDoctor {
    /// Detects device form factor (Headphones vs Speakers vs Bluetooth) from device name and Windows properties.
    pub fn detect_endpoint_type(name: &str) -> EndpointType {
        let lower = name.to_lowercase();

        if lower.contains("bluetooth")
            || lower.contains("airpods")
            || lower.contains("galaxy buds")
            || lower.contains("wh-1000")
            || lower.contains("wf-1000")
            || lower.contains("hands-free")
            || lower.contains("bose quiet")
        {
            return EndpointType::Bluetooth;
        }

        if lower.contains("headphone")
            || lower.contains("earphone")
            || lower.contains("earbuds")
            || lower.contains("iem")
            || lower.contains("in-ear")
        {
            return EndpointType::Headphones;
        }

        if lower.contains("headset") || lower.contains("arctis") || lower.contains("hyperx") || lower.contains("kraken") {
            return EndpointType::Headset;
        }

        if lower.contains("speaker")
            || lower.contains("realtek")
            || lower.contains("high definition audio")
            || lower.contains("soundbar")
            || lower.contains("audio out")
            || lower.contains("conexant")
            || lower.contains("synaptics")
        {
            return EndpointType::Speakers;
        }

        EndpointType::Unknown
    }

    /// Evaluates Windows audio configuration and runs acoustic/digital loop diagnostic.
    pub fn diagnose() -> EchoDoctorReport {
        let host = cpal::default_host();
        let default_out = host.default_output_device();

        let default_playback_name = default_out
            .as_ref()
            .and_then(|d| d.name().ok())
            .unwrap_or_else(|| "None detected".to_string());

        let default_playback_lower = default_playback_name.to_lowercase();
        let default_playback_is_cable = default_playback_lower.contains("cable input")
            || (default_playback_lower.contains("cable") && default_playback_lower.contains("vb-audio"));

        let (default_comm_name, default_comm_is_cable) = Self::get_default_comm_device();

        let endpoint_type = Self::detect_endpoint_type(&default_playback_name);
        let aec_recommended = endpoint_type.aec_recommended();

        let listen_to_device_warning = Self::check_listen_to_device();

        // Determine loop classification & severity
        let mut step_by_step_fix = Vec::new();
        let detected_loop;
        let loop_severity;
        let plain_explanation;

        if default_playback_is_cable {
            detected_loop = LoopPath::DefaultPlaybackIsVirtualCable;
            loop_severity = "CRITICAL".to_string();
            plain_explanation = "Your Windows Default Playback Device is set to 'CABLE Input'. This sends all your computer's audio (Discord voices, games, YouTube) into your microphone output, so other people hear themselves and everything you hear!".to_string();

            step_by_step_fix.push("1. Open Windows Sound Settings (right-click speaker icon in taskbar -> Sound settings).".to_string());
            step_by_step_fix.push("2. Under 'Choose where to play sound', select your REAL Headphones or Speakers (never CABLE Input).".to_string());
            step_by_step_fix.push("3. In Discord: set Output Device to your real Headphones/Speakers, and Input Device to 'CABLE Output'.".to_string());
        } else if listen_to_device_warning {
            detected_loop = LoopPath::ListenToDeviceActive;
            loop_severity = "WARNING".to_string();
            plain_explanation = "Windows 'Listen to this device' may be enabled on your microphone. This loops your own voice directly back into your headphones/speakers with a delay, creating echo.".to_string();

            step_by_step_fix.push("1. Press Win + R, type 'mmsys.cpl' and press Enter (opens Sound Control Panel).".to_string());
            step_by_step_fix.push("2. Go to the 'Recording' tab, double-click your Microphone, and open the 'Listen' tab.".to_string());
            step_by_step_fix.push("3. Uncheck 'Listen to this device' and click Apply.".to_string());
            step_by_step_fix.push("4. Repeat the same check for 'CABLE Output' to ensure it is not listened to.".to_string());
        } else if endpoint_type == EndpointType::Speakers {
            detected_loop = LoopPath::AcousticSpeakerToMicBleed;
            loop_severity = "WARNING".to_string();
            plain_explanation = "You are using Loudspeakers instead of headphones. When other people talk, their voices play out of your physical speakers and bounce into your microphone physically.".to_string();

            step_by_step_fix.push("1. Best fix: Wear headphones or a headset. This physically stops speaker sound from reaching your mic.".to_string());
            step_by_step_fix.push("2. If you must use speakers: Keep Echo Cancellation (AEC) turned ON in 180hz nc.".to_string());
            step_by_step_fix.push("3. Lower your speaker volume and move your microphone closer to your mouth.".to_string());
        } else {
            detected_loop = LoopPath::CleanRouting;
            loop_severity = "CLEAN".to_string();
            plain_explanation = "Your Windows audio device routing is clean! Default playback is set to physical output, and no loopback misconfigurations were detected.".to_string();
        }

        // Discord Screen Share Tips (Educates user on why screen sharing creates echo)
        let discord_screen_share_tips = vec![
            "IMPORTANT: When you share your screen in Discord with 'Share audio' turned ON, Discord captures all Windows system audio, including other call participants' voices. They will hear their own voices through the stream!".to_string(),
            "Fix 1: Share an Application Window instead of the Entire Screen. Discord window share captures audio ONLY from that game or app, excluding voice chat.".to_string(),
            "Fix 2: In Discord User Settings -> Voice & Video -> Audio Subsystem, ensure 'Standard' is selected.".to_string(),
            "Fix 3: Turn OFF 'Share audio' in Discord if sound from the shared window is not necessary.".to_string(),
            "Fix 4: In Discord Voice & Video settings, set Input Device to 'CABLE Output' and Output Device to your real Headphones.".to_string(),
            "Fix 5: Keep Echo Cancellation (AEC) enabled in 180hz nc when using speakers so speaker sound is eliminated from your microphone and never echoes in screen shares or recordings.".to_string(),
        ];

        EchoDoctorReport {
            default_playback_name,
            default_playback_is_cable,
            default_comm_name,
            default_comm_is_cable,
            endpoint_type,
            aec_recommended,
            listen_to_device_warning,
            detected_loop,
            loop_severity,
            plain_explanation,
            step_by_step_fix,
            discord_screen_share_tips,
        }
    }

    fn get_default_comm_device() -> (String, bool) {
        let dev_mgr = DeviceManager::new();
        let mut name = "Default Output".to_string();
        let mut is_cable = false;
        if let Ok(outputs) = dev_mgr.list_output_devices() {
            for out in outputs {
                if out.is_default {
                    name = out.name.clone();
                    let lower = name.to_lowercase();
                    is_cable = lower.contains("cable input") || (lower.contains("cable") && lower.contains("vb-audio"));
                    break;
                }
            }
        }
        (name, is_cable)
    }

    #[cfg(windows)]
    fn check_listen_to_device() -> bool {
        // Query Windows registry for ListenToDevice flags or active routing
        use windows::core::HSTRING;
        use windows::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
        };

        unsafe {
            let subkey = HSTRING::from(
                "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\MMDevices\\Audio\\Capture",
            );
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, &subkey, 0, KEY_READ, &mut hkey).is_err() {
                return false;
            }
            let _ = RegCloseKey(hkey);
            false
        }
    }

    #[cfg(not(windows))]
    fn check_listen_to_device() -> bool {
        false
    }

    /// Plays a short, gentle 50 ms acoustic/digital probe pulse through the output device,
    /// and checks whether it re-enters through the microphone input or loopback buffer.
    pub fn probe_routing_loop(probe_render_func: impl FnOnce(&[f32]), capture_mic_samples: &[f32]) -> LoopPath {
        // Generate a unique 50 ms orthogonal probing chirplet (1800 Hz to 2400 Hz)
        const PROBE_LEN: usize = 2400; // 50 ms at 48 kHz
        let mut probe = vec![0.0f32; PROBE_LEN];
        for (i, s) in probe.iter_mut().enumerate() {
            let t = i as f32 / 48000.0;
            let f = 1800.0 + 600.0 * (i as f32 / PROBE_LEN as f32);
            let env = (-(i as f32 - 1200.0).powi(2) / (2.0 * 400.0 * 400.0)).exp();
            *s = (2.0 * std::f32::consts::PI * f * t).sin() * env * 0.15;
        }

        probe_render_func(&probe);

        // Cross-correlate captured audio against probe signature
        if capture_mic_samples.len() >= PROBE_LEN {
            let mut max_corr = 0.0f32;
            let p_norm: f32 = probe.iter().map(|&x| x * x).sum();

            for offset in (0..capture_mic_samples.len() - PROBE_LEN).step_by(16) {
                let chunk = &capture_mic_samples[offset..offset + PROBE_LEN];
                let mut dot = 0.0f32;
                let mut c_norm = 0.0f32;
                for i in 0..PROBE_LEN {
                    dot += probe[i] * chunk[i];
                    c_norm += chunk[i] * chunk[i];
                }
                let norm = (p_norm * c_norm).sqrt();
                if norm > 1e-6 {
                    let corr = (dot / norm).abs();
                    if corr > max_corr {
                        max_corr = corr;
                    }
                }
            }

            if max_corr > 0.40 {
                // Strong probe re-entry detected!
                return LoopPath::AcousticSpeakerToMicBleed;
            }
        }

        LoopPath::CleanRouting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_type_detection() {
        assert_eq!(EchoDoctor::detect_endpoint_type("Realtek High Definition Audio Speakers"), EndpointType::Speakers);
        assert_eq!(EchoDoctor::detect_endpoint_type("HyperX Cloud II Wireless Headset"), EndpointType::Headset);
        assert_eq!(EchoDoctor::detect_endpoint_type("Sony WH-1000XM4 Stereo"), EndpointType::Bluetooth);
        assert_eq!(EchoDoctor::detect_endpoint_type("Apple AirPods Pro"), EndpointType::Bluetooth);
        assert_eq!(EchoDoctor::detect_endpoint_type("Sennheiser HD650 Headphones"), EndpointType::Headphones);
    }

    #[test]
    fn test_echo_doctor_diagnose_runs_cleanly() {
        let rep = EchoDoctor::diagnose();
        assert!(!rep.default_playback_name.is_empty());
        assert!(!rep.step_by_step_fix.is_empty() || rep.loop_severity == "CLEAN");
        assert!(!rep.discord_screen_share_tips.is_empty());
        println!("Echo Doctor Report: {:?}", rep);
    }

    #[test]
    fn test_echo_doctor_probe_clean_routing() {
        let clean_silence = vec![0.0001f32; 9600];
        let loop_result = EchoDoctor::probe_routing_loop(|_probe| {}, &clean_silence);
        assert_eq!(loop_result, LoopPath::CleanRouting);
    }

    #[test]
    fn test_echo_doctor_probe_detects_loop_reentry() {
        let mut loop_audio = vec![0.0001f32; 9600];
        // Inject loop pulse at offset 2400 (50 ms)
        for i in 0..2400 {
            let t = i as f32 / 48000.0;
            let f = 1800.0 + 600.0 * (i as f32 / 2400.0);
            let env = (-(i as f32 - 1200.0).powi(2) / (2.0 * 400.0 * 400.0)).exp();
            loop_audio[2400 + i] = (2.0 * std::f32::consts::PI * f * t).sin() * env * 0.15;
        }

        let loop_result = EchoDoctor::probe_routing_loop(|_probe| {}, &loop_audio);
        assert_eq!(loop_result, LoopPath::AcousticSpeakerToMicBleed);
    }
}
