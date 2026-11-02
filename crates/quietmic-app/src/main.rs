#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use parking_lot::Mutex;
use quietmic_core::audio::{AudioDeviceInfo, DeviceManager, VirtualDeviceStatus};
use quietmic_core::config::AppConfig;
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
}

// ----------------------------------------------------------------------------
// Tauri Commands
// ----------------------------------------------------------------------------

#[tauri::command]
fn get_input_devices(state: State<AppState>) -> Result<Vec<AudioDeviceInfo>, String> {
    state.device_manager.list_input_devices()
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

    match AudioStreamController::start(&state.device_manager, cfg.selected_mic.as_deref(), pipeline, speaker_pipe) {
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

    let initial_controller = match AudioStreamController::start(&device_manager, config.selected_mic.as_deref(), pipeline, speaker_pipe) {
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
    };

    tauri::Builder::default()
        .manage(app_state)
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
            let toggle_item = MenuItem::with_id(app, "toggle_bypass", "Toggle Active/Bypass (Ctrl+Shift+B)", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "open_settings", "Settings...", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit QuietMic", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&toggle_item, &settings_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("QuietMic - Real-Time AI Noise Suppression")
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
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
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("Error running QuietMic application");
}
