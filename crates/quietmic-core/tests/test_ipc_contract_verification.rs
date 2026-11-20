use quietmic_core::config::AppConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
struct CommandContract {
    #[serde(default)]
    args: HashMap<String, String>,
    #[serde(default)]
    returns: Option<String>,
    #[serde(default)]
    range: Option<Vec<f64>>,
    #[serde(default)]
    allowed: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EventContract {
    #[serde(default)]
    payload: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ConfigContract {
    #[serde(rename = "type")]
    field_type: String,
    #[serde(default)]
    default: Value,
    #[serde(default)]
    range: Option<Vec<f64>>,
    #[serde(default)]
    allowed: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct IpcContract {
    commands: HashMap<String, CommandContract>,
    events: HashMap<String, EventContract>,
    config: HashMap<String, ConfigContract>,
}

fn load_contract() -> IpcContract {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let contract_path = Path::new(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("ipc-contract.json");
    assert!(
        contract_path.exists(),
        "ipc-contract.json must exist at {}",
        contract_path.display()
    );

    let content = fs::read_to_string(&contract_path).expect("Failed to read ipc-contract.json");
    serde_json::from_str(&content).expect("Failed to parse ipc-contract.json")
}

#[test]
fn test_ipc_contract_file_validity() {
    let contract = load_contract();
    assert!(
        !contract.commands.is_empty(),
        "Contract must have commands registered"
    );
    assert!(
        contract.commands.contains_key("set_noise_mode"),
        "Must contain set_noise_mode"
    );
    assert!(
        contract.commands.contains_key("set_strength"),
        "Must contain set_strength"
    );
    assert!(
        contract.commands.contains_key("set_bypass"),
        "Must contain set_bypass"
    );
    assert!(
        contract.commands.contains_key("set_aec"),
        "Must contain set_aec"
    );
    assert!(
        contract.commands.contains_key("get_metrics"),
        "Must contain get_metrics"
    );
    assert!(
        contract.commands.contains_key("get_config"),
        "Must contain get_config"
    );
    assert!(
        contract.commands.contains_key("set_ptt_state"),
        "Must contain set_ptt_state"
    );
}

#[test]
fn test_scan_frontend_invoke_calls_against_contract() {
    let contract = load_contract();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let app_js_path = Path::new(manifest_dir)
        .parent()
        .unwrap()
        .join("quietmic-app")
        .join("ui")
        .join("app.js");
    assert!(
        app_js_path.exists(),
        "app.js must exist at {}",
        app_js_path.display()
    );

    let js_content = fs::read_to_string(&app_js_path).expect("Failed to read app.js");

    // Scan for invoke('command', ...) calls
    let mut search_pos = 0;
    let mut invoked_commands = Vec::new();

    while let Some(idx) = js_content[search_pos..].find("invoke('") {
        let start = search_pos + idx + "invoke('".len();
        if let Some(end) = js_content[start..].find('\'') {
            let cmd = &js_content[start..start + end];
            invoked_commands.push((cmd.to_string(), start));
            search_pos = start + end;
        } else {
            break;
        }
    }

    assert!(
        !invoked_commands.is_empty(),
        "Should find invoke calls in app.js"
    );

    // Verify all invoked commands exist in the contract
    for (cmd, pos) in &invoked_commands {
        assert!(
            contract.commands.contains_key(cmd),
            "Frontend invokes command '{}' at byte offset {} which is NOT in ipc-contract.json!",
            cmd,
            pos
        );
    }

    // Verify specific arguments for known calls
    // set_ptt_state must use active: bool, not transmitting
    assert!(
        !js_content.contains("invoke('set_ptt_state', { transmitting:"),
        "Frontend must not call set_ptt_state with 'transmitting' argument (must be 'active')"
    );

    // Verify events: check if listen/emit is used in app.js and if so, must be in contract.events
    let mut search_pos = 0;
    while let Some(idx) = js_content[search_pos..].find("listen('") {
        let start = search_pos + idx + "listen('".len();
        if let Some(end) = js_content[start..].find('\'') {
            let event = &js_content[start..start + end];
            assert!(
                contract.events.contains_key(event),
                "Frontend listens to event '{}' which is NOT in ipc-contract.json!",
                event
            );
            search_pos = start + end;
        } else {
            break;
        }
    }
}

/// Simulated Mock IPC dispatcher that enforces contract type and range checks
struct MockIpcHarness {
    contract: IpcContract,
}

impl MockIpcHarness {
    fn new(contract: IpcContract) -> Self {
        Self { contract }
    }

    fn dispatch(&self, cmd: &str, args: &HashMap<String, Value>) -> Result<(), String> {
        let cmd_contract = self
            .contract
            .commands
            .get(cmd)
            .ok_or_else(|| format!("Unknown command: {}", cmd))?;

        // Check required arguments
        for (arg_name, arg_type) in &cmd_contract.args {
            let is_optional = arg_type.starts_with("Option<");
            if let Some(val) = args.get(arg_name) {
                if !val.is_null() {
                    // Check type
                    match arg_type.as_str() {
                        "bool" => {
                            if !val.is_boolean() {
                                return Err(format!("Arg '{}' must be bool, got {:?}", arg_name, val));
                            }
                        }
                        "f32" | "f64" => {
                            if !val.is_number() {
                                return Err(format!("Arg '{}' must be number, got {:?}", arg_name, val));
                            }
                            let num = val.as_f64().unwrap();
                            if let Some(ref range) = cmd_contract.range {
                                if num < range[0] || num > range[1] {
                                    return Err(format!(
                                        "Arg '{}' value {} out of range [{}, {}]",
                                        arg_name, num, range[0], range[1]
                                    ));
                                }
                            }
                        }
                        "String" => {
                            if !val.is_string() {
                                return Err(format!("Arg '{}' must be string, got {:?}", arg_name, val));
                            }
                            let s = val.as_str().unwrap();
                            if let Some(ref allowed) = cmd_contract.allowed {
                                if !allowed.iter().any(|a| a == s) {
                                    return Err(format!(
                                        "Arg '{}' value '{}' not in allowed values: {:?}",
                                        arg_name, s, allowed
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            } else if !is_optional {
                return Err(format!("Missing required argument: {}", arg_name));
            }
        }

        // Check for unexpected extra arguments
        for arg_name in args.keys() {
            if !cmd_contract.args.contains_key(arg_name) {
                return Err(format!(
                    "Unexpected argument '{}' for command '{}'",
                    arg_name, cmd
                ));
            }
        }

        Ok(())
    }
}

#[test]
fn test_mock_ipc_valid_calls() {
    let contract = load_contract();
    let harness = MockIpcHarness::new(contract);

    // Test mode changes
    for mode in ["None", "Light", "Standard", "MaxQuality", "Krisp", "VoiceIsolation"] {
        let mut args = HashMap::new();
        args.insert("mode".to_string(), Value::String(mode.to_string()));
        assert!(harness.dispatch("set_noise_mode", &args).is_ok());
        assert!(harness.dispatch("set_mode", &args).is_ok());
    }

    // Test strength slider across range [0.0, 1.0]
    for &strength in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut args = HashMap::new();
        args.insert("strength".to_string(), serde_json::json!(strength));
        assert!(harness.dispatch("set_strength", &args).is_ok());
    }

    // Test bypass toggle
    for &bypass in &[true, false] {
        let mut args = HashMap::new();
        args.insert("bypass".to_string(), Value::Bool(bypass));
        assert!(harness.dispatch("set_bypass", &args).is_ok());
    }

    // Test aec toggle
    for &enabled in &[true, false] {
        let mut args = HashMap::new();
        args.insert("enabled".to_string(), Value::Bool(enabled));
        assert!(harness.dispatch("set_aec", &args).is_ok());
    }

    // Test set_ptt_state with active
    let mut ptt_args = HashMap::new();
    ptt_args.insert("active".to_string(), Value::Bool(true));
    assert!(harness.dispatch("set_ptt_state", &ptt_args).is_ok());

    // Test theme switching
    for theme in ["night-lab", "daylight", "amber-console", "dark", "light"] {
        let mut args = HashMap::new();
        args.insert("theme".to_string(), Value::String(theme.to_string()));
        assert!(harness.dispatch("set_theme", &args).is_ok());
    }
}

#[test]
fn test_mock_ipc_rejection_of_invalid_calls() {
    let contract = load_contract();
    let harness = MockIpcHarness::new(contract);

    // 1. Unknown command must fail
    assert!(harness.dispatch("nonexistent_command", &HashMap::new()).is_err());

    // 2. Out of range strength must fail
    let mut invalid_strength = HashMap::new();
    invalid_strength.insert("strength".to_string(), serde_json::json!(1.5));
    assert!(harness.dispatch("set_strength", &invalid_strength).is_err());

    let mut negative_strength = HashMap::new();
    negative_strength.insert("strength".to_string(), serde_json::json!(-0.1));
    assert!(harness.dispatch("set_strength", &negative_strength).is_err());

    // 3. Invalid noise mode must fail
    let mut invalid_mode = HashMap::new();
    invalid_mode.insert("mode".to_string(), Value::String("SuperUltraMode".to_string()));
    assert!(harness.dispatch("set_noise_mode", &invalid_mode).is_err());

    // 4. Invalid argument name for set_ptt_state must fail
    let mut bad_ptt = HashMap::new();
    bad_ptt.insert("transmitting".to_string(), Value::Bool(true));
    assert!(harness.dispatch("set_ptt_state", &bad_ptt).is_err());

    // 5. Invalid theme name must fail
    let mut bad_theme = HashMap::new();
    bad_theme.insert("theme".to_string(), Value::String("neon-glow".to_string()));
    assert!(harness.dispatch("set_theme", &bad_theme).is_err());
}

#[test]
fn test_contract_matches_app_config_defaults() {
    let contract = load_contract();
    let default_config = AppConfig::default();

    // Verify mode default
    let mode_cfg = contract.config.get("mode").expect("mode in contract config");
    assert_eq!(
        mode_cfg.default.as_str().unwrap(),
        default_config.mode.as_str()
    );

    // Verify strength default
    let strength_cfg = contract.config.get("strength").expect("strength in contract config");
    assert!((strength_cfg.default.as_f64().unwrap() as f32 - default_config.strength).abs() < 1e-4);

    // Verify bypass default
    let bypass_cfg = contract.config.get("bypass").expect("bypass in contract config");
    assert_eq!(bypass_cfg.default.as_bool().unwrap(), default_config.bypass);

    // Verify aec_enabled default
    let aec_cfg = contract.config.get("aec_enabled").expect("aec_enabled in contract config");
    assert_eq!(aec_cfg.default.as_bool().unwrap(), default_config.aec_enabled);

    // Verify theme default
    let theme_cfg = contract.config.get("theme").expect("theme in contract config");
    assert_eq!(theme_cfg.default.as_str().unwrap(), default_config.theme);

    // Verify close_to_tray default
    let tray_cfg = contract.config.get("close_to_tray").expect("close_to_tray in contract config");
    assert_eq!(tray_cfg.default.as_bool().unwrap(), default_config.close_to_tray);
}
