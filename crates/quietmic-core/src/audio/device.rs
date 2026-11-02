use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub default_sample_rate: u32,
    pub channels: u16,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualDeviceStatus {
    pub is_installed: bool,
    pub cable_input_name: Option<String>,
    pub cable_output_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceManager;

impl DeviceManager {
    pub fn new() -> Self {
        Self
    }

    /// Lists all available audio input (microphone) devices.
    pub fn list_input_devices(&self) -> Result<Vec<AudioDeviceInfo>, String> {
        let host = cpal::default_host();
        let default_device_name = host.default_input_device()
            .and_then(|d| d.name().ok());

        let mut devices = Vec::new();
        let input_devices = host.input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?;

        for dev in input_devices {
            if let Ok(name) = dev.name() {
                // Filter out VB-CABLE Output from the mic selection list to avoid feedback loops
                if name.to_lowercase().contains("cable output") {
                    continue;
                }

                let default_config = dev.default_input_config().ok();
                let sample_rate = default_config.as_ref().map(|c| c.sample_rate().0).unwrap_or(48000);
                let channels = default_config.as_ref().map(|c| c.channels()).unwrap_or(1);
                let is_default = default_device_name.as_deref() == Some(&name);

                devices.push(AudioDeviceInfo {
                    id: name.clone(),
                    name,
                    default_sample_rate: sample_rate,
                    channels,
                    is_default,
                });
            }
        }

        Ok(devices)
    }

    /// Checks whether VB-CABLE virtual audio device is installed and ready.
    /// In VB-CABLE architecture:
    /// - "CABLE Input" is a Playback device (where QuietMic writes cleaned audio).
    /// - "CABLE Output" is a Recording device (where Zoom/Meet/Discord select as mic).
    pub fn check_virtual_cable(&self) -> VirtualDeviceStatus {
        let host = cpal::default_host();
        let mut cable_input = None;
        let mut cable_output = None;

        // Check output devices (QuietMic sends cleaned audio here)
        if let Ok(outputs) = host.output_devices() {
            for dev in outputs {
                if let Ok(name) = dev.name() {
                    let lower = name.to_lowercase();
                    if lower.contains("cable input") || (lower.contains("cable") && lower.contains("vb-audio")) {
                        cable_input = Some(name);
                        break;
                    }
                }
            }
        }

        // Check input devices (Zoom/Meet receive audio from here)
        if let Ok(inputs) = host.input_devices() {
            for dev in inputs {
                if let Ok(name) = dev.name() {
                    let lower = name.to_lowercase();
                    if lower.contains("cable output") || (lower.contains("cable") && lower.contains("vb-audio")) {
                        cable_output = Some(name);
                        break;
                    }
                }
            }
        }

        let is_installed = cable_input.is_some() || cable_output.is_some();
        VirtualDeviceStatus {
            is_installed,
            cable_input_name: cable_input,
            cable_output_name: cable_output,
        }
    }

    /// Finds a named input device or falls back to default.
    pub fn get_input_device(&self, name_opt: Option<&str>) -> Result<cpal::Device, String> {
        let host = cpal::default_host();
        if let Some(target_name) = name_opt {
            if let Ok(inputs) = host.input_devices() {
                for dev in inputs {
                    if let Ok(name) = dev.name() {
                        if name == target_name {
                            return Ok(dev);
                        }
                    }
                }
            }
        }

        host.default_input_device()
            .ok_or_else(|| "No default audio input device found".to_string())
    }

    /// Finds the VB-CABLE input device (render endpoint) to write cleaned audio into.
    pub fn get_virtual_cable_input(&self) -> Result<cpal::Device, String> {
        let host = cpal::default_host();
        let outputs = host.output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?;

        for dev in outputs {
            if let Ok(name) = dev.name() {
                let lower = name.to_lowercase();
                if lower.contains("cable input") || (lower.contains("cable") && lower.contains("vb-audio")) {
                    return Ok(dev);
                }
            }
        }

        Err("VB-CABLE ('CABLE Input') not detected. Please install VB-CABLE virtual audio driver.".to_string())
    }
}


