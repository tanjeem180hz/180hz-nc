use quietmic_core::config::AppConfig;

#[test]
fn test_close_to_tray_defaults_and_serialization() {
    let cfg = AppConfig::default();
    assert!(cfg.close_to_tray, "Default close_to_tray must be true");
    assert!(!cfg.shown_tray_toast, "Default shown_tray_toast must be false");
    assert!(!cfg.start_with_windows, "Default start_with_windows must be false");

    // Test JSON serialization & round-trip
    let json = serde_json::to_string(&cfg).expect("Config should serialize to JSON");
    let deserialized: AppConfig = serde_json::from_str(&json).expect("Config should deserialize from JSON");
    assert_eq!(deserialized.close_to_tray, true);
    assert_eq!(deserialized.shown_tray_toast, false);
    assert_eq!(deserialized.start_with_windows, false);

    // Test backward compatibility when keys are absent
    let legacy_json = r#"{
        "mode": "Krisp",
        "strength": 1.0,
        "bypass": false
    }"#;
    let legacy_cfg: AppConfig = serde_json::from_str(legacy_json).expect("Legacy JSON should deserialize with defaults");
    assert!(legacy_cfg.close_to_tray, "Missing close_to_tray must default to true");
    assert!(!legacy_cfg.shown_tray_toast, "Missing shown_tray_toast must default to false");
    assert!(!legacy_cfg.start_with_windows, "Missing start_with_windows must default to false");
}

#[test]
fn test_close_to_tray_mutation_preservation() {
    let mut cfg = AppConfig::default();
    cfg.close_to_tray = false;
    cfg.shown_tray_toast = true;
    cfg.start_with_windows = true;

    let json = serde_json::to_string_pretty(&cfg).expect("Serialization failed");
    let reloaded: AppConfig = serde_json::from_str(&json).expect("Deserialization failed");

    assert!(!reloaded.close_to_tray, "close_to_tray must remain false after reload");
    assert!(reloaded.shown_tray_toast, "shown_tray_toast must remain true after reload");
    assert!(reloaded.start_with_windows, "start_with_windows must remain true after reload");
}

#[cfg(windows)]
#[test]
fn test_windows_autostart_lifecycle() {
    // 1. Initial cleanup
    let _ = AppConfig::set_windows_autostart(false);
    let initial_status = AppConfig::get_windows_autostart_status();
    assert!(!initial_status.enabled, "Autostart should be disabled initially");

    // 2. Enable autostart
    assert!(AppConfig::set_windows_autostart(true).is_ok(), "Setting autostart true must succeed");
    let enabled_status = AppConfig::get_windows_autostart_status();
    assert!(enabled_status.enabled, "Autostart must report enabled = true");

    // 3. Disable autostart
    assert!(AppConfig::set_windows_autostart(false).is_ok(), "Setting autostart false must succeed");
    let disabled_status = AppConfig::get_windows_autostart_status();
    assert!(!disabled_status.enabled, "Autostart must report enabled = false");
}

#[test]
fn test_minimized_arg_detection() {
    let args_with_minimized = vec!["180hz nc.exe".to_string(), "--minimized".to_string()];
    let is_minimized = args_with_minimized.iter().any(|a| a == "--minimized");
    assert!(is_minimized, "--minimized flag must be detected");

    let args_without = vec!["180hz nc.exe".to_string()];
    let is_not_minimized = args_without.iter().any(|a| a == "--minimized");
    assert!(!is_not_minimized, "Standard launch must not be minimized");
}
