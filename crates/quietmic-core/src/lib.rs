pub mod audio;
pub mod config;
pub mod dsp;
pub mod engine;

pub use audio::{AudioDeviceInfo, AudioMetrics, AudioStreamController, DeviceManager, VirtualDeviceStatus};
pub use config::AppConfig;
pub use dsp::{
    AudioResampler, ComfortNoiseGenerator, DriftCompensator, GainSmoother, HighPassFilter,
    SilenceDetector, SoftLimiter,
};
pub use engine::{
    generate_diagnostic_test_signal, run_diagnostic_sweep, AudioPipeline, DenoiseEngine,
    DiagnosticController, DiagnosticTapProducers, EchoCanceller, EngineMode, LightEngine,
    StageMetrics, StandardEngine, VoiceIsolationEngine, STAGE_NAMES,
};

/// Initializes rotating file logger in %APPDATA%/QuietMic/logs/quietmic.log
pub fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = AppConfig::config_dir().join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(log_dir, "quietmic.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
    guard
}
