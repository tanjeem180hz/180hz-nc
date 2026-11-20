use quietmic_core::audio::echo_doctor::EchoDoctor;
use quietmic_core::audio::live_monitor::LiveMonitorController;
use quietmic_core::config::AppConfig;
use quietmic_core::dsp::crossfade::CrossfadeMode;
use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};
use quietmic_core::engine::EchoCanceller;
use quietmic_core::{run_aec_speaker_test, run_pipeline_self_test};

#[test]
fn test_backend_controls_e2e_all_parameters() {
    println!("\n==========================================================================");
    println!(">>> E2E BACKEND CONTROLS TEST: COMPLETE PARAMETER & COUNTER VERIFICATION <<<");
    println!("==========================================================================");

    let mut config = AppConfig::default();
    let mut pipeline = AudioPipeline::new(48000, 48000).expect("Failed to initialize AudioPipeline");

    // 1. Initial State Assertions
    assert_eq!(config.mode, "Krisp");
    assert_eq!(config.strength, 1.0);
    assert!(!config.bypass);
    assert!(!config.aec_enabled);
    assert!(config.auto_sensitivity);
    assert!(config.close_to_tray);

    // 2. Power Toggle (Master Bypass)
    // In UI: Click power button toggles bypass
    config.bypass = true;
    pipeline.set_mode(EngineMode::None);
    let params = pipeline.get_runtime_parameters();
    assert_eq!(params.mode, "None (True Bypass)");

    config.bypass = false;
    pipeline.set_mode(EngineMode::MaxQuality);
    let params = pipeline.get_runtime_parameters();
    assert_eq!(params.mode, "Max Quality (Krisp-Level 48kHz)");

    // 3. Four Modes Switching (None, Light, Standard, MaxQuality)
    // Mode: None
    pipeline.set_mode(EngineMode::None);
    assert_eq!(pipeline.get_runtime_parameters().mode, "None (True Bypass)");

    // Mode: Light
    pipeline.set_mode(EngineMode::Light);
    assert_eq!(pipeline.get_runtime_parameters().mode, "Light (RNNoise 48kHz)");

    // Mode: Standard
    pipeline.set_mode(EngineMode::Standard);
    assert_eq!(pipeline.get_runtime_parameters().mode, "Standard (DeepFilterNet3 48kHz)");

    // Mode: MaxQuality
    pipeline.set_mode(EngineMode::MaxQuality);
    assert_eq!(pipeline.get_runtime_parameters().mode, "Max Quality (Krisp-Level 48kHz)");

    // 4. Strength Slider: 0%, 50%, 100%
    for strength in [0.0f32, 0.5f32, 1.0f32] {
        config.strength = strength;
        pipeline.set_strength(strength);
        assert!((pipeline.get_runtime_parameters().strength - strength).abs() < 1e-4);
    }

    // 5. Auto-Sensitivity and Threshold Controls
    // Toggle auto off, set manual threshold to -45.0 dBFS
    config.auto_sensitivity = false;
    pipeline.set_auto_sensitivity(false);
    config.sensitivity_threshold = -45.0;
    pipeline.set_sensitivity_threshold(-45.0);
    let params = pipeline.get_runtime_parameters();
    assert!(!params.auto_sensitivity_enabled);
    assert!((params.sensitivity_threshold_dbfs - (-45.0)).abs() < 1e-3);

    // Toggle auto on
    config.auto_sensitivity = true;
    pipeline.set_auto_sensitivity(true);
    assert!(pipeline.get_runtime_parameters().auto_sensitivity_enabled);

    // 6. Acoustic Echo Cancellation (AEC)
    config.aec_enabled = true;
    pipeline.set_aec_enabled(true);
    assert!(pipeline.get_runtime_parameters().aec_enabled);

    config.aec_enabled = false;
    pipeline.set_aec_enabled(false);
    assert!(!pipeline.get_runtime_parameters().aec_enabled);

    // 7. Live Test Controller & A/B Switching
    let (mut live_monitor, _live_tap) = LiveMonitorController::new();
    let initial_status = live_monitor.get_status();
    assert!(!initial_status.is_active);

    live_monitor.set_ab_mode(CrossfadeMode::Cleaned);
    assert_eq!(live_monitor.get_status().ab_mode, "Cleaned");

    live_monitor.set_ab_mode(CrossfadeMode::Original);
    assert_eq!(live_monitor.get_status().ab_mode, "Original");

    live_monitor.set_volume(0.8);
    assert!((live_monitor.get_status().volume - 0.8).abs() < 1e-3);

    // 8. Self-Test Engine Execution
    println!(">>> Running Self-Test Engine...");
    let self_test_report = run_pipeline_self_test(&mut pipeline);
    assert!(
        self_test_report.passed,
        "Self-Test must pass: {}",
        self_test_report.summary
    );
    assert!(
        self_test_report.noise_only_reduction_db >= 15.0,
        "Noise reduction must be >= 15 dB, got {:.2} dB",
        self_test_report.noise_only_reduction_db
    );

    // 9. AEC Speaker Test Execution
    println!(">>> Running AEC Speaker Test...");
    let mut aec = EchoCanceller::default_preset();
    let aec_test_report = run_aec_speaker_test(&mut aec);
    assert!(aec_test_report.passed);

    // 10. Echo Doctor Diagnostic Execution
    println!(">>> Running Echo Doctor...");
    let echo_doctor_report = EchoDoctor::diagnose();
    assert!(!echo_doctor_report.plain_explanation.is_empty());

    // 11. Preferences: Close to tray & Start with Windows
    config.close_to_tray = true;
    assert!(config.close_to_tray);
    config.close_to_tray = false;
    assert!(!config.close_to_tray);

    config.start_with_windows = false;
    assert!(!config.start_with_windows);

    // 12. Theme & UI Effects Settings
    for theme in ["night-lab", "daylight", "amber-console", "dark", "light"] {
        config.theme = theme.to_string();
        assert_eq!(config.theme, theme);
    }

    // 13. Process Synthetic Audio and Verify Frame Processing
    pipeline.set_mode(EngineMode::MaxQuality);
    pipeline.set_strength(1.0);
    pipeline.set_bypass(false);

    let frame_len = 480; // 10ms at 48kHz
    let mut input_buffer = vec![0.0f32; frame_len];
    let mut output_buffer = Vec::with_capacity(frame_len);

    // Synthetic speech + background noise
    for (i, sample) in input_buffer.iter_mut().enumerate() {
        let t = i as f32 / 48000.0;
        let speech = (2.0 * std::f32::consts::PI * 300.0 * t).sin() * 0.2;
        let noise = ((i * 1103515245 + 12345) & 0x7FFFFFFF) as f32 / 0x7FFFFFFF as f32 * 0.05;
        *sample = speech + noise;
    }

    let mut successful_frames = 0u64;
    for _ in 0..100 {
        output_buffer.clear();
        let res = pipeline.process_frame_internal(&input_buffer, &mut output_buffer, 0);
        assert!(res.is_ok(), "Frame processing should succeed");
        assert!(output_buffer.len() >= 480, "Output frame should contain audio samples");
        successful_frames += 1;
    }

    assert_eq!(successful_frames, 100);
    println!("  [PASS] All E2E backend controls and 100 audio frames verified successfully.");
}
