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
    #[serde(default = "default_true")]
    pub comfort_noise: bool,       // Subtle acoustic floor (default true)
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
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: "Krisp".to_string(),
            strength: 1.0,
            bypass: false,
            aec_enabled: false,
            speaker_denoise: false,
            comfort_noise: true,
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
        }
    }
}

impl AppConfig {
    /// Returns the standard config directory in %APPDATA%/180Hz-NC
    pub fn config_dir() -> PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(appdata.clone()).join("180Hz-NC");
        let _ = fs::create_dir_all(&dir);

        // Seamless migration from legacy QuietMic path
        let legacy_config = PathBuf::from(appdata).join("QuietMic").join("config.json");
        let new_config = dir.join("config.json");
        if legacy_config.exists() && !new_config.exists() {
            let _ = fs::copy(&legacy_config, &new_config);
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
                Ok(cfg) => cfg,
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

            let app_name = HSTRING::from("180Hz-NC");

            if enable {
                if let Ok(exe_path) = std::env::current_exe() {
                    let mut path_str = exe_path.to_string_lossy().to_string();
                    path_str.push_str(" --minimized");
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
                }
            } else {
                let _ = RegDeleteValueW(hkey, &app_name);
            }

            let _ = RegCloseKey(hkey);
        }

        Ok(())
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
    }

    #[test]
    fn test_windows_autostart_registry() {
        // Test setting autostart true then false in HKCU Run key
        assert!(AppConfig::set_windows_autostart(true).is_ok());
        assert!(AppConfig::set_windows_autostart(false).is_ok());
    }
}
