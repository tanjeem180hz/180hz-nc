#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use parking_lot::Mutex;
use quietmic_core::audio::{
    AudioDeviceInfo, DeviceManager, LiveMonitorController, LiveMonitorStatus, VirtualDeviceStatus,
};
use quietmic_core::config::AppConfig;
use quietmic_core::dsp::crossfade::CrossfadeMode;
use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};
use quietmic_core::{init_logging, AudioStreamController};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, State};

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND};
#[cfg(windows)]
use windows::Win32::System::Threading::CreateMutexW;
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_SHIFT, MOD_NOREPEAT,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsDto {
    pub frame_time_us: u64,
    pub input_peak_db: f32,
    pub output_peak_db: f32,
    pub underruns: u64,
    pub overruns: u64,
}

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub device_manager: DeviceManager,
    pub stream_controller: Mutex<Option<AudioStreamController>>,
    pub live_monitor: Mutex<LiveMonitorController>,
}

// ----------------------------------------------------------------------------
// Tauri Commands
// ----------------------------------------------------------------------------

#[tauri::command]
fn get_input_devices(state: State<AppState>) -> Result<Vec<AudioDeviceInfo>, String> {
    state.device_manager.list_input_devices()
}

#[tauri::command]
fn get_output_devices(state: State<AppState>) -> Result<Vec<AudioDeviceInfo>, String> {
    state.device_manager.list_output_devices()
}

#[tauri::command]
fn start_live_test(device_name: Option<String>, state: State<AppState>) -> Result<(), String> {
    let mut monitor = state.live_monitor.lock();
    monitor.start(&state.device_manager, device_name.as_deref())
}

#[tauri::command]
fn stop_live_test(state: State<AppState>) -> Result<(), String> {
    let mut monitor = state.live_monitor.lock();
    monitor.stop();
    Ok(())
}

#[tauri::command]
fn toggle_live_test(device_name: Option<String>, state: State<AppState>) -> Result<bool, String> {
    let mut monitor = state.live_monitor.lock();
    let current_status = monitor.get_status();
    if current_status.is_active {
        monitor.stop();
        Ok(false)
    } else {
        monitor.start(&state.device_manager, device_name.as_deref())?;
        Ok(true)
    }
}

#[tauri::command]
fn set_live_test_ab(mode: String, state: State<AppState>) -> Result<(), String> {
    let monitor = state.live_monitor.lock();
    let crossfade_mode = match mode.as_str() {
        "Original" => CrossfadeMode::Original,
        _ => CrossfadeMode::Cleaned,
    };
    monitor.set_ab_mode(crossfade_mode);
    Ok(())
}

#[tauri::command]
fn set_live_test_volume(volume: f32, state: State<AppState>) -> Result<(), String> {
    let monitor = state.live_monitor.lock();
    monitor.set_volume(volume);
    Ok(())
}

#[tauri::command]
fn start_record_and_replay(state: State<AppState>) -> Result<(), String> {
    let monitor = state.live_monitor.lock();
    monitor.start_recording()
}

#[tauri::command]
fn start_replay_playback(state: State<AppState>) -> Result<(), String> {
    let mut monitor = state.live_monitor.lock();
    monitor.start_replay()
}

#[tauri::command]
fn export_recorded_wav(dir: Option<String>, state: State<AppState>) -> Result<(String, String), String> {
    let monitor = state.live_monitor.lock();
    let target_dir = if let Some(d) = dir {
        std::path::PathBuf::from(d)
    } else {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join("recordings")
    };
    let (raw, clean) = monitor.export_recorded_wavs(&target_dir)?;
    Ok((raw.to_string_lossy().to_string(), clean.to_string_lossy().to_string()))
}

#[tauri::command]
fn get_live_test_status(state: State<AppState>) -> LiveMonitorStatus {
    let mut monitor = state.live_monitor.lock();
    monitor.get_status()
}

#[tauri::command]
fn run_pipeline_diagnostics(
    output_dir: Option<String>,
    state: State<AppState>,
) -> Result<Vec<quietmic_core::StageMetrics>, String> {
    let out = if let Some(d) = output_dir {
        std::path::PathBuf::from(d)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("diagnostics_output")
    };
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    let test_audio = quietmic_core::generate_diagnostic_test_signal(3.0);

    let cfg = state.config.lock();
    let mut pipeline = AudioPipeline::new(48000, 48000)?;
    let mode = match cfg.mode.as_str() {
        "Light" => EngineMode::Light,
        "VoiceIsolation" => EngineMode::VoiceIsolation,
        _ => EngineMode::Standard,
    };
    pipeline.set_mode(mode);
    pipeline.set_strength(cfg.strength);
    pipeline.set_aec_enabled(cfg.aec_enabled);
    pipeline.set_comfort_noise_enabled(cfg.comfort_noise);

    quietmic_core::run_diagnostic_sweep(&mut pipeline, &test_audio, &out)
}

#[tauri::command]
fn get_system_audio_diagnostics() -> quietmic_core::audio::SystemAudioDiagnostic {
    quietmic_core::audio::SystemAudioDiagnostic::run()
}

#[tauri::command]
fn get_virtual_status(state: State<AppState>) -> VirtualDeviceStatus {
    state.device_manager.check_virtual_cable()
}

#[tauri::command]
fn get_config(state: State<AppState>) -> AppConfig {
    state.config.lock().clone()
}

#[tauri::command]
fn set_mode(mode: String, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.mode = mode.clone();
    let _ = cfg.save();

    // Reconfigure active audio stream
    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_strength(strength: f32, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.strength = strength.clamp(0.0, 1.0);
    let _ = cfg.save();

    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_bypass(bypass: bool, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.bypass = bypass;
    let _ = cfg.save();

    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_input_device(name: Option<String>, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.selected_mic = name;
    let _ = cfg.save();

    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_aec(enabled: bool, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.aec_enabled = enabled;
    let _ = cfg.save();

    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_comfort_noise(enabled: bool, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.comfort_noise = enabled;
    let _ = cfg.save();

    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_speaker_denoise(enabled: bool, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.speaker_denoise = enabled;
    let _ = cfg.save();
    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_muted(muted: bool, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.muted = muted;
    let _ = cfg.save();
    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_autostart(enabled: bool, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.start_with_windows = enabled;
    let _ = cfg.save();
    let _ = AppConfig::set_windows_autostart(enabled);
    Ok(())
}

#[tauri::command]
fn get_metrics(state: State<AppState>) -> MetricsDto {
    let lock = state.stream_controller.lock();
    if let Some(ref ctrl) = *lock {
        let m = ctrl.metrics();
        MetricsDto {
            frame_time_us: m.get_frame_time_us(),
            input_peak_db: m.get_input_peak_db(),
            output_peak_db: m.get_output_peak_db(),
            underruns: m.underruns.load(Ordering::Relaxed),
            overruns: m.overruns.load(Ordering::Relaxed),
        }
    } else {
        MetricsDto {
            frame_time_us: 0,
            input_peak_db: -100.0,
            output_peak_db: -100.0,
            underruns: 0,
            overruns: 0,
        }
    }
}

#[tauri::command]
fn open_url(url: String) {
    #[cfg(windows)]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", &url])
        .spawn();
}

fn restart_audio_stream(state: &AppState, cfg: &AppConfig) -> Result<(), String> {
    let mut stream_lock = state.stream_controller.lock();
    // Stop previous controller cleanly
    if let Some(mut prev) = stream_lock.take() {
        prev.stop();
    }

    let mut pipeline = AudioPipeline::new(48000, 48000)?;
    let mode = match cfg.mode.as_str() {
        "Light" => EngineMode::Light,
        "VoiceIsolation" => EngineMode::VoiceIsolation,
        _ => EngineMode::Standard,
    };
    pipeline.set_mode(mode);
    pipeline.set_strength(cfg.strength);
    pipeline.set_bypass(cfg.bypass);
    pipeline.set_muted(cfg.muted);
    pipeline.set_aec_enabled(cfg.aec_enabled);
    pipeline.set_comfort_noise_enabled(cfg.comfort_noise);

    // Optional Speaker Denoise pipeline
    let speaker_pipe = if cfg.speaker_denoise {
        let mut spk = AudioPipeline::new(48000, 48000)?;
        spk.set_mode(mode);
        spk.set_strength(cfg.strength);
        spk.set_bypass(cfg.bypass);
        Some(spk)
    } else {
        None
    };

    let live_tap = state.live_monitor.lock().create_new_tap();

    match AudioStreamController::start(&state.device_manager, cfg.selected_mic.as_deref(), pipeline, speaker_pipe, Some(live_tap)) {
        Ok(ctrl) => {
            *stream_lock = Some(ctrl);
            Ok(())
        }
        Err(err) => {
            tracing::error!("Failed to restart audio stream: {}", err);
            Err(err)
        }
    }
}

// ----------------------------------------------------------------------------
// Application Entry Point
// ----------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--diagnostic" || a == "--diagnostics" || a == "-d") {
        let out_dir = args
            .iter()
            .position(|a| a == "--diagnostic" || a == "--diagnostics" || a == "-d")
            .and_then(|idx| args.get(idx + 1))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("diagnostics_output"));

        println!("QuietMic Diagnostic Mode Active");
        println!("Writing 8-stage WAV dumps to: {}", out_dir.display());
        let _ = std::fs::create_dir_all(&out_dir);

        let test_audio = quietmic_core::generate_diagnostic_test_signal(3.0);

        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_mode(EngineMode::Standard);
        pipeline.set_strength(1.0);

        match quietmic_core::run_diagnostic_sweep(&mut pipeline, &test_audio, &out_dir) {
            Ok(metrics) => {
                println!("{:<32} | {:<10} | {:<10} | {:<8} | {:<8}", "Stage Name", "RMS (dB)", "Peak (dB)", "Rate (Hz)", "Ch");
                println!("{:-<76}", "");
                for m in metrics {
                    println!("{:<32} | {:>8.1} dB | {:>8.1} dB | {:>9} | {:>4}", m.stage_name, m.rms_db, m.peak_db, m.sample_rate, m.channels);
                }
                println!("\nDiagnostic dump complete. All 8 WAV stages saved successfully.");
                return;
            }
            Err(e) => {
                eprintln!("Diagnostic sweep failed: {}", e);
                return;
            }
        }
    }

    #[cfg(windows)]
    unsafe {
        let _mutex = CreateMutexW(None, true, w!("Global\\QuietMic_SingleInstance_Mutex"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            eprintln!("QuietMic is already running. Another instance cannot be started.");
            return;
        }
    }

    let _log_guard = init_logging();
    tracing::info!("Starting QuietMic application");

    let config = AppConfig::load();
    let device_manager = DeviceManager::new();

    // Initialize initial audio pipeline
    let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline initialization failed");
    let mode = match config.mode.as_str() {
        "Light" => EngineMode::Light,
        "VoiceIsolation" => EngineMode::VoiceIsolation,
        _ => EngineMode::Standard,
    };
    pipeline.set_mode(mode);
    pipeline.set_strength(config.strength);
    pipeline.set_bypass(config.bypass);
    pipeline.set_muted(config.muted);
    pipeline.set_aec_enabled(config.aec_enabled);
    pipeline.set_comfort_noise_enabled(config.comfort_noise);

    let speaker_pipe = if config.speaker_denoise {
        let mut spk = AudioPipeline::new(48000, 48000).ok();
        if let Some(ref mut s) = spk {
            s.set_mode(mode);
            s.set_strength(config.strength);
            s.set_bypass(config.bypass);
        }
        spk
    } else {
        None
    };

    let (live_monitor, live_tap) = LiveMonitorController::new();

    let initial_controller = match AudioStreamController::start(&device_manager, config.selected_mic.as_deref(), pipeline, speaker_pipe, Some(live_tap)) {
        Ok(ctrl) => Some(ctrl),
        Err(err) => {
            tracing::warn!("Standby mode: Audio device not ready on launch: {}", err);
            None
        }
    };

    let app_state = AppState {
        config: Mutex::new(config),
        device_manager,
        stream_controller: Mutex::new(initial_controller),
        live_monitor: Mutex::new(live_monitor),
    };

    tauri::Builder::default()
        .manage(app_state)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<AppState>();
                state.live_monitor.lock().stop();
            }
        })
        .setup(|app| {
            // Spawn Windows Global Hotkey Listener (Ctrl+Shift+B for Bypass, Ctrl+Shift+M for Mute)
            #[cfg(windows)]
            {
                let app_handle = app.handle().clone();
                std::thread::Builder::new()
                    .name("quietmic-hotkeys".to_string())
                    .spawn(move || unsafe {
                        let _ = RegisterHotKey(HWND::default(), 1, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, 0x42);
                        let _ = RegisterHotKey(HWND::default(), 2, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, 0x4D);

                        let mut msg = MSG::default();
                        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                            if msg.message == WM_HOTKEY {
                                let hotkey_id = msg.wParam.0 as i32;
                                let state = app_handle.state::<AppState>();
                                if hotkey_id == 1 {
                                    let mut cfg = state.config.lock();
                                    cfg.bypass = !cfg.bypass;
                                    let _ = cfg.save();
                                    let _ = restart_audio_stream(&state, &cfg);
                                    tracing::info!("Global Hotkey: Bypass toggled to {}", cfg.bypass);
                                } else if hotkey_id == 2 {
                                    let mut cfg = state.config.lock();
                                    cfg.muted = !cfg.muted;
                                    let _ = cfg.save();
                                    let _ = restart_audio_stream(&state, &cfg);
                                    tracing::info!("Global Hotkey: Mute toggled to {}", cfg.muted);
                                }
                            }
                        }

                        let _ = UnregisterHotKey(HWND::default(), 1);
                        let _ = UnregisterHotKey(HWND::default(), 2);
                    })
                    .expect("Failed to spawn global hotkey thread");
            }

            // Spawn Audio Device Hot-plug Watcher
            {
                let app_handle = app.handle().clone();
                std::thread::Builder::new()
                    .name("quietmic-hotplug-watcher".to_string())
                    .spawn(move || {
                        let mut last_dev_count = 0usize;
                        loop {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            let state = app_handle.state::<AppState>();
                            if let Ok(devs) = state.device_manager.list_input_devices() {
                                let count = devs.len();
                                if last_dev_count != 0 && count != last_dev_count {
                                    tracing::info!("Audio device hot-plug detected: count changed from {} to {}", last_dev_count, count);
                                    let cfg = state.config.lock().clone();
                                    let _ = restart_audio_stream(&state, &cfg);
                                }
                                last_dev_count = count;
                            }
                        }
                    })
                    .expect("Failed to spawn hot-plug watcher");
            }

            // Build System Tray Menu
            let live_test_item = MenuItem::with_id(app, "toggle_live_test", "Live Test: On/Off", true, None::<&str>)?;
            let toggle_item = MenuItem::with_id(app, "toggle_bypass", "Toggle Active/Bypass (Ctrl+Shift+B)", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "open_settings", "Settings...", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit QuietMic", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&live_test_item, &toggle_item, &settings_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("QuietMic - Real-Time AI Noise Suppression")
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "toggle_live_test" => {
                            let state = app.state::<AppState>();
                            let mut monitor = state.live_monitor.lock();
                            let st = monitor.get_status();
                            if st.is_active {
                                monitor.stop();
                            } else {
                                let _ = monitor.start(&state.device_manager, None);
                            }
                        }
                        "toggle_bypass" => {
                            let state = app.state::<AppState>();
                            let mut cfg = state.config.lock();
                            cfg.bypass = !cfg.bypass;
                            let _ = cfg.save();
                            let _ = restart_audio_stream(&state, &cfg);
                        }
                        "open_settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            let state = app.state::<AppState>();
                            state.live_monitor.lock().stop();
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_input_devices,
            get_output_devices,
            get_virtual_status,
            get_config,
            set_mode,
            set_strength,
            set_bypass,
            set_muted,
            set_input_device,
            set_aec,
            set_comfort_noise,
            set_speaker_denoise,
            set_autostart,
            get_metrics,
            start_live_test,
            stop_live_test,
            toggle_live_test,
            set_live_test_ab,
            set_live_test_volume,
            start_record_and_replay,
            start_replay_playback,
            export_recorded_wav,
            get_live_test_status,
            run_pipeline_diagnostics,
            get_system_audio_diagnostics,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("Error running QuietMic application");
}
