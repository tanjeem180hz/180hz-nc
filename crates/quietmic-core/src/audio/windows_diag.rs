use crate::audio::device::DeviceManager;
use cpal::traits::HostTrait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAudioDiagnostic {
    pub mic_permission_granted: bool,
    pub virtual_cable_installed: bool,
    pub virtual_cable_input_found: bool,
    pub virtual_cable_output_found: bool,
    pub default_input_name: String,
    pub default_input_sample_rate: u32,
    pub default_output_name: String,
    pub default_output_sample_rate: u32,
    pub recommendations: Vec<String>,
}

impl SystemAudioDiagnostic {
    pub fn run() -> Self {
        let dev_mgr = DeviceManager::new();
        let cable_status = dev_mgr.check_virtual_cable();

        let host = cpal::default_host();
        let default_in = host.default_input_device();
        let default_out = host.default_output_device();

        let default_input_name = default_in
            .as_ref()
            .and_then(|d| cpal::traits::DeviceTrait::name(d).ok())
            .unwrap_or_else(|| "None detected".to_string());

        let default_input_sample_rate = default_in
            .as_ref()
            .and_then(|d| cpal::traits::DeviceTrait::default_input_config(d).ok())
            .map(|c| c.sample_rate().0)
            .unwrap_or(0);

        let default_output_name = default_out
            .as_ref()
            .and_then(|d| cpal::traits::DeviceTrait::name(d).ok())
            .unwrap_or_else(|| "None detected".to_string());

        let default_output_sample_rate = default_out
            .as_ref()
            .and_then(|d| cpal::traits::DeviceTrait::default_output_config(d).ok())
            .map(|c| c.sample_rate().0)
            .unwrap_or(0);

        let mic_permission_granted = check_microphone_permission();

        let mut recommendations = Vec::new();

        if !mic_permission_granted {
            recommendations.push(
                "Windows microphone privacy permission is set to Deny. Please enable microphone access in Windows Settings -> Privacy & Security -> Microphone.".to_string(),
            );
        }

        if !cable_status.is_installed {
            recommendations.push(
                "VB-CABLE Driver is not detected. Please install VB-Audio Virtual Cable (from https://vb-audio.com/Cable/) and select 'CABLE Output' as your microphone in Zoom/Discord/Teams.".to_string(),
            );
        }

        if default_input_sample_rate != 0 && default_input_sample_rate != 48000 {
            recommendations.push(format!(
                "Microphone sample rate is currently {} Hz. 48000 Hz is recommended for zero resampling overhead and lowest latency.",
                default_input_sample_rate
            ));
        }

        // Always provide clear third-party meeting app guidance
        recommendations.push(
            "Conflict Prevention: If using Discord, Zoom, Google Meet, or Teams, disable their built-in noise suppression (Krisp/RNNoise) to avoid double-denoising and muffled voice.".to_string(),
        );

        Self {
            mic_permission_granted,
            virtual_cable_installed: cable_status.is_installed,
            virtual_cable_input_found: cable_status.cable_input_name.is_some(),
            virtual_cable_output_found: cable_status.cable_output_name.is_some(),
            default_input_name,
            default_input_sample_rate,
            default_output_name,
            default_output_sample_rate,
            recommendations,
        }
    }
}

#[cfg(windows)]
fn check_microphone_permission() -> bool {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_SZ,
    };

    unsafe {
        let subkey = HSTRING::from(
            "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone",
        );
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, 0, KEY_READ, &mut hkey).is_err() {
            // Key might not exist if user never modified consent -> defaults to granted
            return true;
        }

        let val_name = HSTRING::from("Value");
        let mut buffer = [0u16; 64];
        let mut buf_size = (buffer.len() * 2) as u32;
        let mut val_type = REG_SZ;

        let query_res = RegQueryValueExW(
            hkey,
            &val_name,
            None,
            Some(&mut val_type),
            Some(buffer.as_mut_ptr() as *mut u8),
            Some(&mut buf_size),
        );

        let _ = RegCloseKey(hkey);

        if query_res.is_ok() {
            let str_val = String::from_utf16_lossy(&buffer);
            !str_val.to_lowercase().starts_with("deny")
        } else {
            true
        }
    }
}

#[cfg(not(windows))]
fn check_microphone_permission() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_audio_diagnostic_run() {
        let diag = SystemAudioDiagnostic::run();
        assert!(!diag.recommendations.is_empty());
        println!("Diagnostic Result: {:?}", diag);
    }
}
