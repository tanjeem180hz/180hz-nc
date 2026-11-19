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
    pub frames_received: u64,
    pub frames_processed: u64,
    pub frames_passthrough: u64,
    pub fallback_active: bool,
    pub noise_reduction_db: f32,
    pub suppression_active: bool,
    pub aec_enabled: bool,
    pub aec_ref_device: String,
    pub aec_ref_sample_rate: u32,
    pub aec_ref_peak_db: f32,
    pub aec_frames_processed: u64,
    pub aec_erle_db: f32,
    pub aec_delay_ms: f32,
    pub aec_endpoint: String,
    pub loop_warning: bool,
    pub raw_waveform: Vec<f32>,
    pub clean_waveform: Vec<f32>,
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
        // Ensure main audio capture pipeline is active so mic frames are fed to monitor
        {
            let ctrl_lock = state.stream_controller.lock();
            if ctrl_lock.is_none() {
                drop(ctrl_lock);
                let cfg = state.config.lock().clone();
                let _ = restart_audio_stream(&state, &cfg);
            }
        }

        let dev = if let Some(ref d) = device_name {
            if !d.trim().is_empty() {
                Some(d.clone())
            } else {
                state.config.lock().selected_speaker.clone()
            }
        } else {
            state.config.lock().selected_speaker.clone()
        };
        let vol = state.config.lock().speaker_volume;
        monitor.set_volume(vol);
        monitor.start(&state.device_manager, dev.as_deref())?;
        Ok(true)
    }
}

#[tauri::command]
fn set_output_device(name: Option<String>, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.selected_speaker = name.clone();
    let _ = cfg.save();

    let mut monitor = state.live_monitor.lock();
    if monitor.get_status().is_active {
        let vol = cfg.speaker_volume;
        monitor.set_volume(vol);
        let _ = monitor.start(&state.device_manager, name.as_deref());
    }
    Ok(())
}

#[tauri::command]
fn set_mic_volume(volume: f32, state: State<AppState>) -> Result<(), String> {
    let vol = volume.clamp(0.0, 2.0);
    {
        let mut cfg = state.config.lock();
        cfg.mic_volume = vol;
        let _ = cfg.save();
    }
    if let Some(ref ctrl) = *state.stream_controller.lock() {
        ctrl.set_mic_volume(vol);
    }
    Ok(())
}

#[tauri::command]
fn set_speaker_volume(volume: f32, state: State<AppState>) -> Result<(), String> {
    let vol = volume.clamp(0.0, 1.0);
    {
        let mut cfg = state.config.lock();
        cfg.speaker_volume = vol;
        let _ = cfg.save();
    }
    state.live_monitor.lock().set_volume(vol);
    Ok(())
}

#[tauri::command]
fn set_auto_sensitivity(enabled: bool, state: State<AppState>) -> Result<(), String> {
    {
        let mut cfg = state.config.lock();
        cfg.auto_sensitivity = enabled;
        let _ = cfg.save();
    }
    if let Some(ref ctrl) = *state.stream_controller.lock() {
        ctrl.set_auto_sensitivity(enabled);
    }
    Ok(())
}

#[tauri::command]
fn set_sensitivity_threshold(threshold: f32, state: State<AppState>) -> Result<(), String> {
    let thresh = threshold.clamp(-80.0, 0.0);
    {
        let mut cfg = state.config.lock();
        cfg.sensitivity_threshold = thresh;
        let _ = cfg.save();
    }
    if let Some(ref ctrl) = *state.stream_controller.lock() {
        ctrl.set_sensitivity_threshold(thresh);
    }
    Ok(())
}

#[tauri::command]
fn set_noise_mode(mode: String, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    match mode.as_str() {
        "None" | "Bypass" => {
            cfg.bypass = true;
            cfg.mode = "None".to_string();
        }
        "Light" => {
            cfg.bypass = false;
            cfg.mode = "Light".to_string();
        }
        "Standard" => {
            cfg.bypass = false;
            cfg.mode = "Standard".to_string();
        }
        "MaxQuality" | "Max Quality" | "Krisp" => {
            cfg.bypass = false;
            cfg.mode = "MaxQuality".to_string();
        }
        _ => {
            cfg.bypass = false;
            cfg.mode = mode.clone();
        }
    }
    let _ = cfg.save();
    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_push_to_talk(enabled: bool, state: State<AppState>) -> Result<(), String> {
    {
        let mut cfg = state.config.lock();
        cfg.push_to_talk = enabled;
        let _ = cfg.save();
    }
    if let Some(ref ctrl) = *state.stream_controller.lock() {
        ctrl.set_push_to_talk(enabled);
    }
    Ok(())
}

#[tauri::command]
fn set_ptt_key(key: String, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.ptt_key = key;
    let _ = cfg.save();
    Ok(())
}

#[tauri::command]
fn set_ptt_state(active: bool, state: State<AppState>) -> Result<(), String> {
    if let Some(ref ctrl) = *state.stream_controller.lock() {
        ctrl.set_ptt_active(active);
    }
    Ok(())
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
        "None" | "Bypass" => EngineMode::None,
        "Light" => EngineMode::Light,
        "Standard" => EngineMode::Standard,
        "MaxQuality" | "Max Quality" | "Krisp" => EngineMode::MaxQuality,
        "VoiceIsolation" => EngineMode::VoiceIsolation,
        _ => EngineMode::MaxQuality,
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
fn run_pipeline_self_test_command() -> Result<quietmic_core::SelfTestReport, String> {
    let mut pipeline = quietmic_core::AudioPipeline::new(48000, 48000).map_err(|e| e.to_string())?;
    pipeline.set_mode(quietmic_core::EngineMode::Standard);
    pipeline.set_strength(1.0);
    Ok(quietmic_core::run_pipeline_self_test(&mut pipeline))
}

#[tauri::command]
fn run_aec_speaker_test_command() -> Result<quietmic_core::AecSpeakerTestReport, String> {
    let mut aec = quietmic_core::EchoCanceller::default_preset();
    Ok(quietmic_core::run_aec_speaker_test(&mut aec))
}

#[tauri::command]
fn run_echo_doctor_command() -> quietmic_core::audio::EchoDoctorReport {
    quietmic_core::audio::EchoDoctor::diagnose()
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
fn get_engine_runtime_parameters(state: State<AppState>) -> quietmic_core::EngineRuntimeParameters {
    let cfg = state.config.lock();
    let mut pipeline = quietmic_core::AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
    let mode = match cfg.mode.as_str() {
        "None" | "Bypass" => quietmic_core::EngineMode::None,
        "Light" => quietmic_core::EngineMode::Light,
        "Standard" => quietmic_core::EngineMode::Standard,
        "MaxQuality" | "Max Quality" | "Krisp" => quietmic_core::EngineMode::MaxQuality,
        "VoiceIsolation" => quietmic_core::EngineMode::VoiceIsolation,
        _ => quietmic_core::EngineMode::MaxQuality,
    };
    pipeline.set_mode(mode);
    pipeline.set_strength(cfg.strength);
    pipeline.set_bypass(cfg.bypass);
    pipeline.set_aec_enabled(cfg.aec_enabled);
    pipeline.set_auto_sensitivity(cfg.auto_sensitivity);
    pipeline.set_sensitivity_threshold(cfg.sensitivity_threshold);
    pipeline.set_mic_gain(cfg.mic_volume);
    pipeline.get_runtime_parameters()
}

#[tauri::command]
fn set_mode(mode: String, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.mode = mode.clone();
    let _ = cfg.save();
    tracing::info!("Settings -> Engine: mode={}, strength={}, aec={}, auto_sens={}", cfg.mode, cfg.strength, cfg.aec_enabled, cfg.auto_sensitivity);

    // Reconfigure active audio stream
    restart_audio_stream(&state, &cfg)
}

#[tauri::command]
fn set_strength(strength: f32, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.strength = strength.clamp(0.0, 1.0);
    let _ = cfg.save();
    tracing::info!("Settings -> Engine: strength={:.2}", cfg.strength);

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
    let cfg = state.config.lock();
    let suppression_active = !cfg.bypass && !cfg.muted;
    drop(cfg);

    if let Some(ref ctrl) = *lock {
        let m = ctrl.metrics();
        let (raw_wave, clean_wave) = m.get_waveform_snapshot();
        MetricsDto {
            frame_time_us: m.get_frame_time_us(),
            input_peak_db: m.get_input_peak_db(),
            output_peak_db: m.get_output_peak_db(),
            underruns: m.underruns.load(Ordering::Relaxed),
            overruns: m.overruns.load(Ordering::Relaxed),
            frames_received: m.get_frames_received(),
            frames_processed: m.get_frames_processed(),
            frames_passthrough: m.get_frames_passthrough(),
            fallback_active: m.is_fallback_active(),
            noise_reduction_db: m.get_noise_reduction_db(),
            suppression_active: suppression_active && !m.is_fallback_active(),
            aec_enabled: m.is_aec_enabled(),
            aec_ref_device: m.get_aec_ref_device_name(),
            aec_ref_sample_rate: m.get_aec_ref_sample_rate(),
            aec_ref_peak_db: m.get_aec_ref_peak_db(),
            aec_frames_processed: m.get_aec_frames_processed(),
            aec_erle_db: m.get_aec_erle_db(),
            aec_delay_ms: m.get_aec_delay_ms(),
            aec_endpoint: m.get_aec_endpoint_type().to_string(),
            loop_warning: m.is_loop_warning_active(),
            raw_waveform: raw_wave,
            clean_waveform: clean_wave,
        }
    } else {
        MetricsDto {
            frame_time_us: 0,
            input_peak_db: -100.0,
            output_peak_db: -100.0,
            underruns: 0,
            overruns: 0,
            frames_received: 0,
            frames_processed: 0,
            frames_passthrough: 0,
            fallback_active: false,
            noise_reduction_db: 0.0,
            suppression_active: false,
            aec_enabled: false,
            aec_ref_device: "None".to_string(),
            aec_ref_sample_rate: 0,
            aec_ref_peak_db: -100.0,
            aec_frames_processed: 0,
            aec_erle_db: 0.0,
            aec_delay_ms: 0.0,
            aec_endpoint: "Unknown".to_string(),
            loop_warning: false,
            raw_waveform: Vec::new(),
            clean_waveform: Vec::new(),
        }
    }
}

#[tauri::command]
fn set_theme(theme: String, state: State<AppState>) -> Result<(), String> {
    let mut cfg = state.config.lock();
    cfg.theme = theme;
    let _ = cfg.save();
    Ok(())
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
        "None" | "Bypass" => EngineMode::None,
        "Light" => EngineMode::Light,
        "Standard" => EngineMode::Standard,
        "MaxQuality" | "Max Quality" | "Krisp" => EngineMode::MaxQuality,
        "VoiceIsolation" => EngineMode::VoiceIsolation,
        _ => EngineMode::MaxQuality,
    };
    pipeline.set_mode(mode);
    pipeline.set_strength(cfg.strength);
    pipeline.set_bypass(cfg.bypass);
    pipeline.set_muted(cfg.muted);
    pipeline.set_mic_gain(cfg.mic_volume);
    pipeline.set_push_to_talk(cfg.push_to_talk);
    pipeline.set_auto_sensitivity(cfg.auto_sensitivity);
    pipeline.set_sensitivity_threshold(cfg.sensitivity_threshold);
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

#[cfg(windows)]
fn parse_vk_code(key_name: &str) -> Option<i32> {
    match key_name.trim().to_lowercase().as_str() {
        "space" => Some(0x20),
        "control" | "ctrl" | "leftctrl" => Some(0x11),
        "shift" | "leftshift" => Some(0x10),
        "alt" | "leftalt" => Some(0x12),
        "capslock" | "caps lock" => Some(0x14),
        "tab" => Some(0x09),
        "enter" | "return" => Some(0x0D),
        "mouse4" | "xbutton1" => Some(0x05),
        "mouse5" | "xbutton2" => Some(0x06),
        "mbutton" | "middlemouse" => Some(0x04),
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap().to_ascii_uppercase();
            if c.is_ascii_alphanumeric() {
                Some(c as i32)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--diagnostic" || a == "--diagnostics" || a == "-d") {
        let out_dir = args
            .iter()
            .position(|a| a == "--diagnostic" || a == "--diagnostics" || a == "-d")
            .and_then(|idx| args.get(idx + 1))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("diagnostics_output"));

        println!("180hz nc Diagnostic Mode Active");
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
        let _mutex = CreateMutexW(None, true, w!("Global\\180hznc_SingleInstance_Mutex"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            eprintln!("180hz nc is already running. Another instance cannot be started.");
            return;
        }
    }

    let _log_guard = init_logging();
    tracing::info!("Starting 180hz nc application");

    let mut config = AppConfig::load();
    let device_manager = DeviceManager::new();

    // Automatically detect and select default physical microphone
    let input_devices = device_manager.list_input_devices().unwrap_or_default();
    let mic_exists = config.selected_mic.as_ref().is_some_and(|m| {
        input_devices.iter().any(|d| d.name == *m)
    });
    if !mic_exists {
        if let Some(default_mic) = input_devices.iter().find(|d| d.is_default).or_else(|| input_devices.first()) {
            config.selected_mic = Some(default_mic.name.clone());
        }
    }

    // Automatically detect and select default physical headphones / speakers
    let output_devices = device_manager.list_output_devices().unwrap_or_default();
    let spk_exists = config.selected_speaker.as_ref().is_some_and(|s| {
        output_devices.iter().any(|d| d.name == *s)
    });
    if !spk_exists {
        if let Some(default_spk) = output_devices.iter().find(|d| d.is_default).or_else(|| output_devices.first()) {
            config.selected_speaker = Some(default_spk.name.clone());
        }
    }
    let _ = config.save();

    // Initialize initial audio pipeline
    let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline initialization failed");
    let mode = match config.mode.as_str() {
        "None" | "Bypass" => EngineMode::None,
        "Light" => EngineMode::Light,
        "Standard" => EngineMode::Standard,
        "MaxQuality" | "Max Quality" | "Krisp" => EngineMode::MaxQuality,
        "VoiceIsolation" => EngineMode::VoiceIsolation,
        _ => EngineMode::MaxQuality,
    };
    pipeline.set_mode(mode);
    pipeline.set_strength(config.strength);
    pipeline.set_bypass(config.bypass);
    pipeline.set_muted(config.muted);
    pipeline.set_mic_gain(config.mic_volume);
    pipeline.set_push_to_talk(config.push_to_talk);
    pipeline.set_auto_sensitivity(config.auto_sensitivity);
    pipeline.set_sensitivity_threshold(config.sensitivity_threshold);
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
    live_monitor.set_volume(config.speaker_volume);

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
                    .name("180hznc-hotkeys".to_string())
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
                    .name("180hznc-hotplug-watcher".to_string())
                    .spawn(move || {
                        let mut last_dev_count = 0usize;
                        loop {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            let state = app_handle.state::<AppState>();
                            if let Ok(devs) = state.device_manager.list_input_devices() {
                                let count = devs.len();
                                if last_dev_count != 0 && count != last_dev_count {
                                    tracing::info!("Audio device hot-plug detected: count changed from {} to {}", last_dev_count, count);
                                    let mut cfg = state.config.lock();
                                    let mic_exists = cfg.selected_mic.as_ref().is_some_and(|m| {
                                        devs.iter().any(|d| d.name == *m)
                                    });
                                    if !mic_exists {
                                        if let Some(def_mic) = devs.iter().find(|d| d.is_default).or_else(|| devs.first()) {
                                            cfg.selected_mic = Some(def_mic.name.clone());
                                        }
                                    }
                                    if let Ok(spks) = state.device_manager.list_output_devices() {
                                        let spk_exists = cfg.selected_speaker.as_ref().is_some_and(|s| {
                                            spks.iter().any(|d| d.name == *s)
                                        });
                                        if !spk_exists {
                                            if let Some(def_spk) = spks.iter().find(|d| d.is_default).or_else(|| spks.first()) {
                                                cfg.selected_speaker = Some(def_spk.name.clone());
                                            }
                                        }
                                    }
                                    let _ = cfg.save();
                                    let cfg_clone = cfg.clone();
                                    drop(cfg);
                                    let _ = restart_audio_stream(&state, &cfg_clone);
                                }
                                last_dev_count = count;
                            }
                        }
                    })
                    .expect("Failed to spawn hot-plug watcher");
            }

            // Spawn Global Push-to-Talk (PTT) Keyboard/Mouse Poller (Active only when PTT is enabled)
            #[cfg(windows)]
            {
                let app_handle = app.handle().clone();
                std::thread::Builder::new()
                    .name("180hznc-global-ptt".to_string())
                    .spawn(move || {
                        use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
                        let state = app_handle.state::<AppState>();
                        let mut was_pressed = false;
                        loop {
                            let (ptt_enabled, ptt_key) = {
                                let cfg = state.config.lock();
                                (cfg.push_to_talk, cfg.ptt_key.clone())
                            };

                            if ptt_enabled {
                                std::thread::sleep(std::time::Duration::from_millis(15));
                                if let Some(vk) = parse_vk_code(&ptt_key) {
                                    let is_down = unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 };
                                    if is_down != was_pressed {
                                        was_pressed = is_down;
                                        if let Some(ref ctrl) = *state.stream_controller.lock() {
                                            ctrl.set_ptt_active(is_down);
                                        }
                                    }
                                }
                            } else {
                                if was_pressed {
                                    was_pressed = false;
                                    if let Some(ref ctrl) = *state.stream_controller.lock() {
                                        ctrl.set_ptt_active(false);
                                    }
                                }
                                std::thread::sleep(std::time::Duration::from_millis(250));
                            }
                        }
                    })
                    .expect("Failed to spawn global PTT thread");
            }

            // Build System Tray Menu
            let live_test_item = MenuItem::with_id(app, "toggle_live_test", "Live Test: On/Off", true, None::<&str>)?;
            let toggle_item = MenuItem::with_id(app, "toggle_bypass", "Toggle Active/Bypass (Ctrl+Shift+B)", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "open_settings", "Settings...", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit 180hz nc", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&live_test_item, &toggle_item, &settings_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("180hz nc")
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
            set_noise_mode,
            set_strength,
            set_bypass,
            set_muted,
            set_input_device,
            set_output_device,
            set_mic_volume,
            set_speaker_volume,
            set_auto_sensitivity,
            set_sensitivity_threshold,
            set_push_to_talk,
            set_ptt_key,
            set_ptt_state,
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
            run_pipeline_self_test_command,
            run_aec_speaker_test_command,
            run_echo_doctor_command,
            get_engine_runtime_parameters,
            set_theme,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("Error running 180hz nc application");
}
