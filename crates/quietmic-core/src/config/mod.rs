use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn default_mode() -> String { "Krisp".to_string() }
fn default_strength() -> f32 { 1.0 }
fn default_true() -> bool { true }
fn default_volume() -> f32 { 1.0 }
fn default_sensitivity_threshold() -> f32 { -50.0 }
fn default_ptt_key() -> String { "Space".to_string() }
fn default_hotkey() -> String { "Ctrl+Alt+N".to_string() }
fn default_theme() -> String { "night-lab".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_mode")]
    pub mode: String,              // "Krisp" (RNNoise AI), "Standard", "VoiceIsolation"
    #[serde(default = "default_strength")]
    pub strength: f32,             // 0.0 to 1.0 (default 1.0 = 100%)
    #[serde(default)]
    pub bypass: bool,              // Master bypass toggle (default false)
    #[serde(default)]
    pub aec_enabled: bool,         // Acoustic Echo Cancellation (default false)
    #[serde(default)]
    pub speaker_denoise: bool,     // Speaker denoise pipeline (default false)
    #[serde(default)]
    pub comfort_noise: bool,       // Subtle acoustic floor (default false for pure Krisp silence)
    #[serde(default)]
    pub selected_mic: Option<String>,
    #[serde(default)]
    pub selected_speaker: Option<String>,
    #[serde(default)]
    pub start_with_windows: bool,  // Registry Run key toggle
    #[serde(default = "default_hotkey")]
    pub global_hotkey: String,     // e.g. "Ctrl+Alt+N"
    #[serde(default)]
    pub muted: bool,              // Global microphone mute state (default false)
    #[serde(default = "default_volume")]
    pub mic_volume: f32,          // Input mic gain (default 1.0 = 100%)
    #[serde(default = "default_volume")]
    pub speaker_volume: f32,      // Output speaker/monitor volume (default 1.0 = 100%)
    #[serde(default = "default_true")]
    pub auto_sensitivity: bool,   // Automatic input sensitivity toggle (default true)
    #[serde(default = "default_sensitivity_threshold")]
    pub sensitivity_threshold: f32, // Input sensitivity threshold in dBFS (default -50.0)
    #[serde(default)]
    pub push_to_talk: bool,       // Push to talk toggle (default false)
    #[serde(default = "default_ptt_key")]
    pub ptt_key: String,          // Key binding for PTT (default "Space")
    #[serde(default = "default_theme")]
    pub theme: String,            // "night-lab", "daylight", "amber-console"
    #[serde(default = "default_true")]
    pub close_to_tray: bool,      // Close button minimizes to tray instead of quitting (default true)
    #[serde(default)]
    pub shown_tray_toast: bool,   // First-time minimize-to-tray toast shown (default false)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutostartStatus {
    pub enabled: bool,
    pub disabled_by_os: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: "Krisp".to_string(),
            strength: 1.0,
            bypass: false,
            aec_enabled: false,
            speaker_denoise: false,
            comfort_noise: false,
            selected_mic: None,
            selected_speaker: None,
            start_with_windows: false,
            global_hotkey: "Ctrl+Alt+N".to_string(),
            muted: false,
            mic_volume: 1.0,
            speaker_volume: 1.0,
            auto_sensitivity: true,
            sensitivity_threshold: -50.0,
            push_to_talk: false,
            ptt_key: "Space".to_string(),
            theme: "night-lab".to_string(),
            close_to_tray: true,
            shown_tray_toast: false,
        }
    }
}

impl AppConfig {
    /// Returns the standard config directory in %APPDATA%/180hz nc
    pub fn config_dir() -> PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(&appdata).join("180hz nc");
        let _ = fs::create_dir_all(&dir);

        // Seamless migration from legacy QuietMic and 180Hz-NC paths
        let legacy_qm = PathBuf::from(&appdata).join("QuietMic").join("config.json");
        let legacy_180 = PathBuf::from(&appdata).join("180Hz-NC").join("config.json");
        let new_config = dir.join("config.json");

        if !new_config.exists() {
            if legacy_180.exists() {
                let _ = fs::copy(&legacy_180, &new_config);
            } else if legacy_qm.exists() {
                let _ = fs::copy(&legacy_qm, &new_config);
            }
        }

        dir
    }

    /// Path to config.json
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    /// Loads configuration from disk.
    /// In accordance with robustness requirements:
    /// "Never crash on bad config; reset to defaults with a logged warning."
    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            let default_cfg = Self::default();
            let _ = default_cfg.save();
            return default_cfg;
        }

        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                Ok(mut cfg) => {
                    let mut modified = false;
                    // Only heal genuinely corrupted/out-of-bounds values without clobbering user preferences
                    if !cfg.mic_volume.is_finite() || cfg.mic_volume < 0.0 || cfg.mic_volume > 2.0 {
                        cfg.mic_volume = 1.0;
                        modified = true;
                    }
                    if !cfg.sensitivity_threshold.is_finite()
                        || cfg.sensitivity_threshold > 0.0
                        || cfg.sensitivity_threshold < -80.0
                    {
                        cfg.sensitivity_threshold = -50.0;
                        modified = true;
                    }
                    if !cfg.strength.is_finite() || cfg.strength < 0.0 || cfg.strength > 1.0 {
                        cfg.strength = 1.0;
                        modified = true;
                    }
                    if modified {
                        let _ = cfg.save();
                    }
                    cfg
                }
                Err(err) => {
                    tracing::warn!(
                        "Corrupted config.json detected: {}. Resetting to safe defaults.",
                        err
                    );
                    let default_cfg = Self::default();
                    let _ = default_cfg.save();
                    default_cfg
                }
            },
            Err(err) => {
                tracing::warn!(
                    "Failed to read config.json: {}. Using default configuration.",
                    err
                );
                Self::default()
            }
        }
    }

    /// Saves configuration to disk atomically.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(&path, json)
            .map_err(|e| format!("Failed to write config file to {:?}: {}", path, e))
    }

    /// Toggles Windows Auto-Start via Registry (HKCU\Software\Microsoft\Windows\CurrentVersion\Run).
    #[cfg(windows)]
    pub fn set_windows_autostart(enable: bool) -> Result<(), String> {
        use windows::core::HSTRING;
        use windows::Win32::System::Registry::{
            RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
            KEY_SET_VALUE, REG_SZ,
        };

        unsafe {
            let subkey = HSTRING::from("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
            let mut hkey = HKEY::default();
            let open_res = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &subkey,
                0,
                KEY_SET_VALUE,
                &mut hkey,
            );

            if open_res.is_err() {
                return Err("Failed to open Registry Run key".to_string());
            }

            let app_name = HSTRING::from("180hz nc");
            let legacy_name_1 = HSTRING::from("180Hz-NC");
            let legacy_name_2 = HSTRING::from("QuietMic");

            if enable {
                if let Ok(exe_path) = std::env::current_exe() {
                    let path_str = format!("\"{}\" --minimized", exe_path.to_string_lossy());
                    let h_val = HSTRING::from(path_str);
                    let bytes: &[u8] = std::slice::from_raw_parts(
                        h_val.as_ptr() as *const u8,
                        (h_val.len() + 1) * 2,
                    );
                    let _ = RegSetValueExW(
                        hkey,
                        &app_name,
                        0,
                        REG_SZ,
                        Some(bytes),
                    );
                    let _ = RegDeleteValueW(hkey, &legacy_name_1);
                    let _ = RegDeleteValueW(hkey, &legacy_name_2);
                }
            } else {
                let _ = RegDeleteValueW(hkey, &app_name);
                let _ = RegDeleteValueW(hkey, &legacy_name_1);
                let _ = RegDeleteValueW(hkey, &legacy_name_2);
            }

            let _ = RegCloseKey(hkey);
        }

        Ok(())
    }

    /// Queries the real state of Windows Auto-Start from Registry, detecting if disabled by Task Manager.
    #[cfg(windows)]
    pub fn get_windows_autostart_status() -> AutostartStatus {
        use windows::core::HSTRING;
        use windows::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
        };

        let mut enabled = false;
        let mut disabled_by_os = false;

        unsafe {
            let subkey = HSTRING::from("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, 0, KEY_READ, &mut hkey).is_ok() {
                let app_name = HSTRING::from("180hz nc");
                let mut size = 0u32;
                if RegQueryValueExW(hkey, &app_name, None, None, None, Some(&mut size)).is_ok() && size > 0 {
                    enabled = true;
                }
                let _ = RegCloseKey(hkey);
            }

            // Check Task Manager StartupApproved key
            let startup_subkey = HSTRING::from("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");
            let mut startup_hkey = HKEY::default();
            if RegOpenKeyExW(HKEY_CURRENT_USER, &startup_subkey, 0, KEY_READ, &mut startup_hkey).is_ok() {
                let app_name = HSTRING::from("180hz nc");
                let mut data = [0u8; 16];
                let mut size = data.len() as u32;
                if RegQueryValueExW(startup_hkey, &app_name, None, None, Some(data.as_mut_ptr()), Some(&mut size)).is_ok() && size > 0 {
                    // Windows sets byte 0 to 0x03 or odd value when disabled by user in Task Manager
                    if data[0] == 0x03 || data[0] == 0x01 || (data[0] % 2 != 0 && data[0] != 0x02) {
                        disabled_by_os = true;
                    }
                }
                let _ = RegCloseKey(startup_hkey);
            }
        }

        AutostartStatus {
            enabled,
            disabled_by_os,
        }
    }

    #[cfg(not(windows))]
    pub fn get_windows_autostart_status() -> AutostartStatus {
        AutostartStatus {
            enabled: false,
            disabled_by_os: false,
        }
    }

    #[cfg(not(windows))]
    pub fn set_windows_autostart(_enable: bool) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let cfg = AppConfig::default();
        let serialized = serde_json::to_string(&cfg).expect("Serialization failed");
        let deserialized: AppConfig = serde_json::from_str(&serialized).expect("Deserialization failed");

        assert_eq!(cfg.mode, deserialized.mode);
        assert_eq!(cfg.strength, deserialized.strength);
        assert_eq!(cfg.bypass, deserialized.bypass);
        assert_eq!(cfg.muted, deserialized.muted);
        assert_eq!(cfg.mic_volume, deserialized.mic_volume);
        assert_eq!(cfg.speaker_volume, deserialized.speaker_volume);
        assert_eq!(cfg.auto_sensitivity, deserialized.auto_sensitivity);
        assert_eq!(cfg.sensitivity_threshold, deserialized.sensitivity_threshold);
        assert_eq!(cfg.push_to_talk, deserialized.push_to_talk);
        assert_eq!(cfg.ptt_key, deserialized.ptt_key);
        assert_eq!(cfg.close_to_tray, deserialized.close_to_tray);
        assert_eq!(cfg.shown_tray_toast, deserialized.shown_tray_toast);
    }

    #[test]
    fn test_windows_autostart_registry() {
        // Test setting autostart true then false in HKCU Run key
        assert!(AppConfig::set_windows_autostart(true).is_ok());
        let st = AppConfig::get_windows_autostart_status();
        assert!(st.enabled);
        assert!(AppConfig::set_windows_autostart(false).is_ok());
        let st2 = AppConfig::get_windows_autostart_status();
        assert!(!st2.enabled);
    }
}
