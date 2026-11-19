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
    pub mic_boost_high_warning: bool,
    pub audio_enhancements_warning: bool,
    pub windows_build_number: u32,
    pub supports_process_loopback: bool,
    pub meeting_app_guidance: String,
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
        let meeting_app_guidance = "In Discord, Zoom, and Teams: Select 'CABLE Output' as your Microphone. Turn OFF their built-in noise suppression (Krisp/RNNoise) and disable 'Automatically adjust mic volume' to prevent fighting 180hz nc.".to_string();
        recommendations.push(meeting_app_guidance.clone());

        let mic_boost_high_warning = false;
        let audio_enhancements_warning = true; // Recommend checking Windows Audio Enhancements
        recommendations.push(
            "Windows Audio Enhancements: Check Sound Control Panel -> Microphone Properties -> Advanced. If 'Enable audio enhancements' is checked, uncheck it to ensure clean raw audio reaches the denoiser.".to_string(),
        );

        let build_number = get_windows_build_number();
        let supports_process_loopback = build_number >= 19041;

        if !supports_process_loopback {
            recommendations.push(format!(
                "Windows Build {} detected. Windows 10 Build 19041+ is recommended for modern per-process loopback isolation.",
                build_number
            ));
        }

        Self {
            mic_permission_granted,
            virtual_cable_installed: cable_status.is_installed,
            virtual_cable_input_found: cable_status.cable_input_name.is_some(),
            virtual_cable_output_found: cable_status.cable_output_name.is_some(),
            default_input_name,
            default_input_sample_rate,
            default_output_name,
            default_output_sample_rate,
            mic_boost_high_warning,
            audio_enhancements_warning,
            windows_build_number: build_number,
            supports_process_loopback,
            meeting_app_guidance,
            recommendations,
        }
    }
}

/// Queries the true Windows NT kernel build number via RtlGetVersion (bypassing application compatibility shims).
#[cfg(windows)]
pub fn get_windows_build_number() -> u32 {
    use windows::Win32::System::SystemInformation::OSVERSIONINFOEXW;
    type RtlGetVersionFn = unsafe extern "system" fn(*mut OSVERSIONINFOEXW) -> i32;

    unsafe {
        let ntdll = windows::Win32::System::LibraryLoader::GetModuleHandleW(windows::core::w!("ntdll.dll"));
        if let Ok(handle) = ntdll {
            if !handle.0.is_null() {
                let func_name = std::ffi::CString::new("RtlGetVersion").unwrap();
                if let Some(proc) = windows::Win32::System::LibraryLoader::GetProcAddress(
                    handle,
                    windows::core::PCSTR(func_name.as_ptr() as *const u8),
                ) {
                    let rtl_get_version: RtlGetVersionFn = std::mem::transmute(proc);
                    let mut os_info: OSVERSIONINFOEXW = std::mem::zeroed();
                    os_info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOEXW>() as u32;
                    if rtl_get_version(&mut os_info) == 0 {
                        return os_info.dwBuildNumber;
                    }
                }
            }
        }
    }
    19041
}

#[cfg(not(windows))]
pub fn get_windows_build_number() -> u32 {
    19041
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
