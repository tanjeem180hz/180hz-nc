use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};

#[test]
fn test_settings_to_engine_parameter_synchronization() {
    println!("\n==========================================================================");
    println!(">>> SETTINGS -> ENGINE VERIFICATION: RUNTIME PARAMETER SYNCHRONIZATION <<<");
    println!("==========================================================================");

    let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");

    // 1. Initial defaults
    pipeline.set_mode(EngineMode::MaxQuality);
    pipeline.set_strength(1.0);
    pipeline.set_aec_enabled(false);
    pipeline.set_auto_sensitivity(true);
    let params = pipeline.get_runtime_parameters();
    assert_eq!(params.mode, "Max Quality (Krisp-Level 48kHz)");
    assert_eq!(params.strength, 1.0);
    assert!(!params.aec_enabled);
    assert!(params.auto_sensitivity_enabled);

    // 2. Change Mode to Standard
    pipeline.set_mode(EngineMode::Standard);
    let params = pipeline.get_runtime_parameters();
    assert_eq!(params.mode, "Standard (DeepFilterNet3 48kHz)");
    assert_eq!(params.attenuation_limit_db, 50.0);
    assert!(params.post_filter_enabled);

    // 3. Change Strength
    pipeline.set_strength(0.75);
    let params = pipeline.get_runtime_parameters();
    assert!((params.strength - 0.75).abs() < 1e-4);

    // 4. Change AEC
    pipeline.set_aec_enabled(true);
    let params = pipeline.get_runtime_parameters();
    assert!(params.aec_enabled);

    // 5. Change Sensitivity Threshold (manual mode)
    pipeline.set_auto_sensitivity(false);
    pipeline.set_sensitivity_threshold(-42.0);
    let params = pipeline.get_runtime_parameters();
    assert!(!params.auto_sensitivity_enabled);
    assert!((params.sensitivity_threshold_dbfs - (-42.0)).abs() < 1e-3);

    // 6. True Bypass mode
    pipeline.set_mode(EngineMode::None);
    let params = pipeline.get_runtime_parameters();
    assert_eq!(params.mode, "None (True Bypass)");

    println!("  [PASS] All settings -> engine runtime parameters verified successfully.");
}
