pub mod audio;
pub mod config;
pub mod dsp;
pub mod engine;

pub use audio::{
    AudioDeviceInfo, AudioMetrics, AudioStreamController, DeviceManager, EchoDoctor,
    EchoDoctorReport, EndpointType, LoopPath, VirtualDeviceStatus,
};
pub use config::AppConfig;
pub use dsp::{
    AudioResampler, ComfortNoiseGenerator, DriftCompensator, GainSmoother, HighPassFilter,
    ResidualSuppressor, SilenceDetector, SoftLimiter, TransientSuppressor,
};
pub use engine::{
    generate_diagnostic_test_signal, run_aec_speaker_test, run_diagnostic_sweep,
    run_pipeline_self_test, AecSpeakerTestReport, AudioPipeline, DenoiseEngine,
    DiagnosticController, DiagnosticTapProducers, EchoCanceller, EngineMode, EngineRuntimeParameters,
    LightEngine, SelfTestReport, StageMetrics, StandardEngine, VoiceIsolationEngine, STAGE_NAMES,
};

/// Initializes rotating file logger in %APPDATA%/180Hz-NC/logs/180hz-nc.log
pub fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = AppConfig::config_dir().join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(log_dir, "180hz nc.log");
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
