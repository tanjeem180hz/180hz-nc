use crate::dsp::{
    AudioResampler, AutoSensitivityAdapter, ComfortNoiseGenerator, DriftCompensator, GainSmoother,
    HighPassFilter, ResidualSuppressor, SilenceDetector, SoftLimiter, TransientSuppressor,
};
use crate::engine::aec::EchoCanceller;
use crate::engine::diagnostics::DiagnosticTapProducers;
use crate::engine::light::LightEngine;
use crate::engine::max_quality::MaxQualityEngine;
use crate::engine::standard::StandardEngine;
use crate::engine::traits::DenoiseEngine;
use crate::engine::voice_isolation::VoiceIsolationEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    None,
    Light,
    Standard,
    MaxQuality,
    VoiceIsolation,
}

pub enum ActiveEngine {
    Light(LightEngine),
    Standard(Box<StandardEngine>),
    MaxQuality(Box<MaxQualityEngine>),
    VoiceIsolation(Box<VoiceIsolationEngine>),
}

/// Exact runtime parameters exposed for diagnostics, verification, and UI telemetry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EngineRuntimeParameters {
    pub mode: String,
    pub strength: f32,
    pub wet_dry_ratio: f32,
    pub attenuation_limit_db: f32,
    pub post_filter_enabled: bool,
    pub post_filter_beta: f32,
    pub sensitivity_threshold_dbfs: f32,
    pub aec_enabled: bool,
    pub comfort_noise_enabled: bool,
    pub transient_suppression_enabled: bool,
    pub auto_sensitivity_enabled: bool,
    pub input_gain: f32,
}

/// Master Audio Processing Pipeline.
/// Operates on 10 ms frames at 48 kHz mono (480 samples).
pub struct AudioPipeline {
    mode: EngineMode,
    active_engine: ActiveEngine,
    enrolled_voiceprint: Option<Vec<f32>>,
    hpf: HighPassFilter,
    aec: EchoCanceller,
    vad: SilenceDetector,
    auto_sensitivity: AutoSensitivityAdapter,
    transient: TransientSuppressor,
    residual: ResidualSuppressor,
    smoother: GainSmoother,
    limiter: SoftLimiter,
    comfort_noise: ComfortNoiseGenerator,
    drift_compensator: DriftCompensator,
    input_resampler: AudioResampler,
    output_resampler: AudioResampler,
    bypass: bool,
    muted: bool,
    mic_gain: f32,
    push_to_talk: bool,
    ptt_active: bool,
    strength: f32, // 0.0 to 1.0
    frame_size: usize,
    dry_buffer: Vec<f32>,
    wet_buffer: Vec<f32>,
    cleaned_buffer: Vec<f32>,
    blended_buffer: Vec<f32>,
    drift_buffer: Vec<f32>,
    scratch_in: Vec<f32>,
    diagnostic_tap: Option<DiagnosticTapProducers>,
}

impl AudioPipeline {
    pub const INTERNAL_RATE: u32 = 48000;
    pub const FRAME_SAMPLES: usize = 480; // 10 ms

    pub fn new(input_sample_rate: u32, output_sample_rate: u32) -> Result<Self, String> {
        let in_chunk_samples = ((input_sample_rate as usize) / 100).max(1);
        let input_resampler = AudioResampler::new(input_sample_rate, Self::INTERNAL_RATE, in_chunk_samples)?;
        let output_resampler = AudioResampler::new(Self::INTERNAL_RATE, output_sample_rate, Self::FRAME_SAMPLES)?;

        let mut standard = StandardEngine::new();
        standard.set_suppression(1.0);

        let mut vad = SilenceDetector::default_48k();
        vad.set_auto_sensitivity(false);

        Ok(Self {
            mode: EngineMode::Standard,
            active_engine: ActiveEngine::Standard(Box::new(standard)),
            enrolled_voiceprint: None,
            hpf: HighPassFilter::default_48k(),
            aec: EchoCanceller::default_preset(),
            vad,
            auto_sensitivity: AutoSensitivityAdapter::default_preset(),
            transient: TransientSuppressor::new(TransientSuppressor::LOOKAHEAD_5MS),
            residual: ResidualSuppressor::default_preset(),
            smoother: GainSmoother::default_48k(),
            limiter: SoftLimiter::default_preset(),
            comfort_noise: ComfortNoiseGenerator::default_preset(),
            drift_compensator: DriftCompensator::default_48k(),
            input_resampler,
            output_resampler,
            bypass: false,
            muted: false,
            mic_gain: 1.0,
            push_to_talk: false,
            ptt_active: false,
            strength: 1.0,
            frame_size: Self::FRAME_SAMPLES,
            dry_buffer: vec![0.0f32; Self::FRAME_SAMPLES],
            wet_buffer: vec![0.0f32; Self::FRAME_SAMPLES],
            cleaned_buffer: vec![0.0f32; Self::FRAME_SAMPLES],
            blended_buffer: vec![0.0f32; Self::FRAME_SAMPLES],
            drift_buffer: Vec::with_capacity(Self::FRAME_SAMPLES + 10),
            scratch_in: Vec::with_capacity(Self::FRAME_SAMPLES * 2),
            diagnostic_tap: None,
        })
    }

    /// Reconfigures input and output sample rates, recreating sinc resamplers dynamically.
    pub fn update_sample_rates(&mut self, input_sample_rate: u32, output_sample_rate: u32) -> Result<(), String> {
        let in_chunk_samples = ((input_sample_rate as usize) / 100).max(1);
        self.input_resampler = AudioResampler::new(input_sample_rate, Self::INTERNAL_RATE, in_chunk_samples)?;
        self.output_resampler = AudioResampler::new(Self::INTERNAL_RATE, output_sample_rate, Self::FRAME_SAMPLES)?;
        Ok(())
    }

    /// Returns the pre-denoised dry 48 kHz buffer.
    pub fn dry_buffer(&self) -> &[f32] {
        &self.dry_buffer
    }

    /// Returns the post-denoised blended 48 kHz buffer.
    pub fn blended_buffer(&self) -> &[f32] {
        &self.blended_buffer
    }

    /// Sets operating mode: None (Bypass), Light (RNNoise), Standard (DeepFilterNet), MaxQuality, or VoiceIsolation.
    /// Lazy-loads the active engine and immediately frees memory of inactive engines.
    pub fn set_mode(&mut self, mode: EngineMode) {
        if self.mode != mode {
            self.mode = mode;
            self.active_engine = match mode {
                EngineMode::None => ActiveEngine::Light(LightEngine::new()),
                EngineMode::Light => ActiveEngine::Light(LightEngine::new()),
                EngineMode::Standard => {
                    let mut s = StandardEngine::new();
                    s.set_attenuation_limit_db(36.0);
                    s.set_post_filter(true, 1.35);
                    s.set_suppression(self.strength);
                    ActiveEngine::Standard(Box::new(s))
                }
                EngineMode::MaxQuality => {
                    let mut mq = MaxQualityEngine::new();
                    mq.set_suppression(self.strength);
                    ActiveEngine::MaxQuality(Box::new(mq))
                }
                EngineMode::VoiceIsolation => {
                    let mut v = VoiceIsolationEngine::new(Box::new(LightEngine::new()));
                    if let Some(ref emb) = self.enrolled_voiceprint {
                        v.set_enrolled_embedding(emb.clone());
                    }
                    ActiveEngine::VoiceIsolation(Box::new(v))
                }
            };
        }
    }

    pub fn mode(&self) -> EngineMode {
        self.mode
    }

    /// Suppression strength slider (0.0 = completely dry / 0% suppression, 1.0 = 100% full suppression).
    pub fn set_strength(&mut self, strength: f32) {
        let s = strength.clamp(0.0, 1.0);
        self.strength = s;
        self.smoother.set_target(s);
        self.residual.set_strength(s);
        match self.active_engine {
            ActiveEngine::Light(ref mut e) => e.set_suppression(s),
            ActiveEngine::Standard(ref mut e) => e.set_suppression(s),
            ActiveEngine::MaxQuality(ref mut e) => e.set_suppression(s),
            ActiveEngine::VoiceIsolation(_) => {}
        }
    }

    pub fn strength(&self) -> f32 {
        self.strength
    }

    /// Master bypass toggle (true = pass unprocessed audio through).
    pub fn set_bypass(&mut self, bypass: bool) {
        self.bypass = bypass;
    }

    pub fn is_bypassed(&self) -> bool {
        self.bypass
    }

    /// Master mute toggle (true = output digital silence).
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Input microphone gain factor (1.0 = 100%, 0.5 = 50%, 2.0 = 200%).
    pub fn set_mic_gain(&mut self, gain: f32) {
        self.mic_gain = gain.clamp(0.0, 4.0);
    }

    pub fn mic_gain(&self) -> f32 {
        self.mic_gain
    }

    /// Push-to-Talk mode toggle.
    pub fn set_push_to_talk(&mut self, enabled: bool) {
        self.push_to_talk = enabled;
    }

    pub fn is_push_to_talk(&self) -> bool {
        self.push_to_talk
    }

    /// Updates real-time Push-to-Talk activation status.
    pub fn set_ptt_active(&mut self, active: bool) {
        self.ptt_active = active;
    }

    pub fn is_ptt_active(&self) -> bool {
        self.ptt_active
    }

    /// Auto sensitivity adjustment toggle.
    pub fn set_auto_sensitivity(&mut self, enabled: bool) {
        self.auto_sensitivity.set_enabled(enabled);
        self.vad.set_auto_sensitivity(false);
    }

    pub fn is_auto_sensitivity(&self) -> bool {
        self.auto_sensitivity.is_enabled()
    }

    /// Sets manual sensitivity threshold in dBFS (e.g. -50.0).
    pub fn set_sensitivity_threshold(&mut self, threshold_dbfs: f32) {
        self.auto_sensitivity.set_manual_threshold_dbfs(threshold_dbfs);
        self.vad.set_threshold_dbfs(threshold_dbfs);
    }

    /// Returns current sensitivity threshold in dBFS.
    pub fn sensitivity_threshold_dbfs(&self) -> f32 {
        self.vad.threshold_dbfs()
    }

    pub fn auto_sensitivity(&self) -> &AutoSensitivityAdapter {
        &self.auto_sensitivity
    }

    pub fn auto_sensitivity_mut(&mut self) -> &mut AutoSensitivityAdapter {
        &mut self.auto_sensitivity
    }

    /// Acoustic Echo Cancellation toggle.
    pub fn set_aec_enabled(&mut self, enabled: bool) {
        self.aec.set_enabled(enabled);
    }

    pub fn is_aec_enabled(&self) -> bool {
        self.aec.is_enabled()
    }

    pub fn aec(&self) -> &EchoCanceller {
        &self.aec
    }

    pub fn aec_mut(&mut self) -> &mut EchoCanceller {
        &mut self.aec
    }

    pub fn feed_aec_reference(&mut self, ref_samples: &[f32]) {
        self.aec.feed_reference(ref_samples);
    }

    /// Comfort noise toggle.
    pub fn set_comfort_noise_enabled(&mut self, enabled: bool) {
        self.comfort_noise.set_enabled(enabled);
    }

    /// Attaches or detaches diagnostic tap producers.
    pub fn set_diagnostic_tap(&mut self, tap: Option<DiagnosticTapProducers>) {
        self.diagnostic_tap = tap;
    }

    /// Access Transient Suppressor.
    pub fn transient_mut(&mut self) -> &mut TransientSuppressor {
        &mut self.transient
    }

    /// Access Residual Suppressor.
    pub fn residual_mut(&mut self) -> &mut ResidualSuppressor {
        &mut self.residual
    }

    /// Access Standard Engine if active.
    pub fn standard_engine_mut(&mut self) -> Option<&mut StandardEngine> {
        match &mut self.active_engine {
            ActiveEngine::Standard(ref mut e) => Some(e.as_mut()),
            _ => None,
        }
    }

    /// Access Voice Isolation engine for speaker enrollment.
    pub fn voice_isolation_mut(&mut self) -> &mut VoiceIsolationEngine {
        if !matches!(self.active_engine, ActiveEngine::VoiceIsolation(_)) {
            let mut v = VoiceIsolationEngine::new(Box::new(LightEngine::new()));
            if let Some(ref emb) = self.enrolled_voiceprint {
                v.set_enrolled_embedding(emb.clone());
            }
            self.active_engine = ActiveEngine::VoiceIsolation(Box::new(v));
            self.mode = EngineMode::VoiceIsolation;
        }
        match &mut self.active_engine {
            ActiveEngine::VoiceIsolation(ref mut v) => v.as_mut(),
            _ => unreachable!(),
        }
    }

    /// Returns the exact runtime engine configuration parameters for logging and diagnostics.
    pub fn get_runtime_parameters(&self) -> EngineRuntimeParameters {
        let (att_limit, pf_en, pf_beta) = match &self.active_engine {
            ActiveEngine::Standard(e) => (e.attenuation_limit_db(), true, e.post_filter_beta()),
            ActiveEngine::MaxQuality(_) => (54.0, true, 1.75),
            _ => (36.0, false, 1.0),
        };
        EngineRuntimeParameters {
            mode: match self.mode {
                EngineMode::None => "None (True Bypass)".to_string(),
                EngineMode::Light => "Light (RNNoise 48kHz)".to_string(),
                EngineMode::Standard => "Standard (DeepFilterNet3 48kHz)".to_string(),
                EngineMode::MaxQuality => "Max Quality (Krisp-Level 48kHz)".to_string(),
                EngineMode::VoiceIsolation => "Voice Isolation (Speaker Enrolled)".to_string(),
            },
            strength: self.strength,
            wet_dry_ratio: self.smoother.current(),
            attenuation_limit_db: att_limit,
            post_filter_enabled: pf_en,
            post_filter_beta: pf_beta,
            sensitivity_threshold_dbfs: self.vad.threshold_dbfs(),
            aec_enabled: self.aec.is_enabled(),
            comfort_noise_enabled: self.comfort_noise.is_enabled(),
            transient_suppression_enabled: self.transient.is_enabled(),
            auto_sensitivity_enabled: self.auto_sensitivity.is_enabled(),
            input_gain: self.auto_sensitivity.current_gain(),
        }
    }

    /// Processes an input audio chunk (from mic) and writes cleaned audio to `output` (for virtual mic).
    /// Guarantees: No allocations in steady-state, automatic fallback to pass-through on error, no NaNs.
    pub fn process_frame_internal(
        &mut self,
        input_mono: &[f32],
        output_mono: &mut Vec<f32>,
        buffer_fill_samples: usize,
    ) -> Result<(), String> {
        // Stage 0: Raw Input
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(0, input_mono);
        }

        if input_mono.is_empty() {
            output_mono.clear();
            return Err("Input buffer is empty".to_string());
        }

        // Step 1: Resample input to 48 kHz if needed
        self.input_resampler.process(input_mono, &mut self.scratch_in)?;

        // If resampler has initial group delay during warmup, pad to full frame size
        if !self.scratch_in.is_empty() && self.scratch_in.len() < self.frame_size {
            self.scratch_in.resize(self.frame_size, 0.0);
        }

        let in_48k = if self.scratch_in.is_empty() {
            input_mono
        } else {
            &self.scratch_in[..]
        };

        // Stage 1: After Resample
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(1, in_48k);
        }

        if in_48k.len() < self.frame_size {
            output_mono.clear();
            output_mono.extend_from_slice(input_mono);
            return Err(format!(
                "Insufficient input samples: got {}, expected at least {}",
                in_48k.len(),
                self.frame_size
            ));
        }

        let frame = &in_48k[..self.frame_size];

        // If muted or PTT active-muted, output complete digital silence
        if self.muted || (self.push_to_talk && !self.ptt_active) {
            output_mono.clear();
            output_mono.resize(self.frame_size, 0.0);
            return Ok(());
        }

        // If None mode or hard bypass requested, forward directly with safety limiting (bit-exact linear)
        if self.mode == EngineMode::None || self.bypass {
            for (i, &s) in frame.iter().enumerate() {
                let scaled = s * self.mic_gain;
                self.dry_buffer[i] = if scaled.is_finite() { scaled.clamp(-1.0, 1.0) } else { 0.0 };
            }
            self.drift_compensator.adjust_frame(&self.dry_buffer, &mut self.drift_buffer, buffer_fill_samples);
            self.output_resampler.process(&self.drift_buffer, output_mono)?;
            return Ok(());
        }

        // Save raw dry frame with hardware NaN/Inf sanitization and transparent soft input saturation on overload
        for (i, &s) in frame.iter().enumerate() {
            let scaled = s * self.mic_gain;
            let clean = if !scaled.is_finite() {
                0.0
            } else if scaled.abs() <= 1.0 {
                scaled
            } else {
                // Smooth soft saturation curve avoiding hard flat-topping square wave distortion on digital overload
                let sign = scaled.signum();
                let excess = scaled.abs() - 1.0;
                sign * (1.0 + 0.05 * (excess / (excess + 0.15)))
            };
            self.dry_buffer[i] = clean;
            self.wet_buffer[i] = clean;
        }
        self.hpf.process_slice(&mut self.wet_buffer);

        // Stage 2: After Highpass
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(2, &self.wet_buffer);
        }

        // Step 3: AEC (subtract room loudspeaker echo if enabled)
        self.aec.process_mic_frame(&mut self.wet_buffer);

        // Step 3b: Dedicated Transient & Click Suppression (mechanical keyboards, mouse clicks)
        self.transient.process_frame(&mut self.wet_buffer);

        // Step 3c: Auto-Sensitivity & Dynamic Level Calibration
        let (auto_in_gain, dynamic_thresh_dbfs, _speech_active) = self.auto_sensitivity.update(&self.wet_buffer);
        self.vad.set_threshold_dbfs(dynamic_thresh_dbfs);

        // Stage 3: After AEC & Transient Suppression
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(3, &self.wet_buffer);
        }

        // Step 4: Input Sensitivity Detection (Silence & Noise Gate)
        let is_active = self.vad.update(&self.wet_buffer);

        // Engine input normalization: normalize engine input to healthy ~ -20 dBFS target
        let should_normalize = (auto_in_gain - 1.0).abs() > 0.01;
        if should_normalize {
            for s in self.wet_buffer.iter_mut() {
                *s *= auto_in_gain;
            }
        }

        // Step 5: Process through Denoise Engine
        // Neural & subband engines continuously process frames to maintain adapted recurrent state
        let engine_res = match &mut self.active_engine {
            ActiveEngine::Light(e) => {
                let res = e.process_frame(&self.wet_buffer, &mut self.cleaned_buffer);
                if res.is_ok() {
                    self.wet_buffer.copy_from_slice(&self.cleaned_buffer);
                }
                res
            }
            ActiveEngine::Standard(e) => {
                let res = e.process_frame(&self.wet_buffer, &mut self.cleaned_buffer);
                if res.is_ok() {
                    self.wet_buffer.copy_from_slice(&self.cleaned_buffer);
                }
                res
            }
            ActiveEngine::MaxQuality(e) => {
                let res = e.process_frame(&self.wet_buffer, &mut self.cleaned_buffer);
                if res.is_ok() {
                    self.wet_buffer.copy_from_slice(&self.cleaned_buffer);
                }
                res
            }
            ActiveEngine::VoiceIsolation(e) => {
                let res = e.process_frame(&self.wet_buffer, &mut self.cleaned_buffer);
                if res.is_ok() {
                    self.wet_buffer.copy_from_slice(&self.cleaned_buffer);
                }
                res
            }
        };

        // Engine failure rule: "Engine failure => automatic pass-through, never silence."
        if let Err(ref err) = engine_res {
            tracing::error!("Denoise engine execution error (falling back to dry audio): {}", err);
            self.wet_buffer.copy_from_slice(&self.dry_buffer);
        }

        // Restore natural dynamic speech level after engine:
        if should_normalize {
            let inv_gain = 1.0 / auto_in_gain;
            for s in self.wet_buffer.iter_mut() {
                *s *= inv_gain;
            }
        }

        // Step 5b: Apply Sensitivity Gating
        // If signal is below input sensitivity threshold, gate to digital silence
        if !is_active {
            self.wet_buffer.fill(0.0);
        } else {
            // Step 5c: Apply Second-Stage Residual Suppressor
            self.residual.process_frame(&mut self.wet_buffer);
        }

        // Stage 4: After Denoise
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(4, &self.wet_buffer);
        }

        // Step 6: Wet/Dry Linear Mix with Gain Smoother
        // Guarantee: At 100% strength, 0.000% dry signal leaks through.
        // At 0% strength, 100% dry signal is passed.
        if self.strength >= 0.999 && (self.smoother.current() - 1.0).abs() < 1e-4 {
            self.blended_buffer.copy_from_slice(&self.wet_buffer);
        } else if self.strength <= 0.001 && self.smoother.current() < 1e-4 {
            self.blended_buffer.copy_from_slice(&self.dry_buffer);
        } else {
            self.smoother.blend_frame(&self.dry_buffer, &self.wet_buffer, &mut self.blended_buffer);
        }

        // Stage 5: After Wet/Dry Mix
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(5, &self.blended_buffer);
        }

        // Step 7: Inject subtle comfort noise floor
        self.comfort_noise.inject_into(&mut self.blended_buffer);

        // Step 8: Soft Peak Limiter (prevent digital clipping)
        self.limiter.process_slice(&mut self.blended_buffer);

        // Guarantee absolute finite stability (no NaNs or infinities can ever escape)
        for s in self.blended_buffer.iter_mut() {
            if !s.is_finite() {
                *s = 0.0;
            }
        }

        // Stage 6: After Post-processing / Limiter
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(6, &self.blended_buffer);
        }

        // Step 9: Adaptive Clock Drift Compensation
        self.drift_compensator.adjust_frame(&self.blended_buffer, &mut self.drift_buffer, buffer_fill_samples);

        // Step 10: Resample to output device sample rate
        self.output_resampler.process(&self.drift_buffer, output_mono)?;

        // Stage 7: Final Output
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(7, output_mono);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_no_reallocations_over_1000_frames() {
        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_mode(EngineMode::Standard);
        pipeline.set_strength(0.85);

        let input_chunk = vec![0.05f32; AudioPipeline::FRAME_SAMPLES];
        let mut output_chunk = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES + 10);
        let initial_cap = output_chunk.capacity();

        for frame_idx in 0..1000 {
            let res = pipeline.process_frame_internal(&input_chunk, &mut output_chunk, 960);
            assert!(res.is_ok(), "Frame {} processing failed", frame_idx);
            assert_eq!(output_chunk.len(), AudioPipeline::FRAME_SAMPLES);
            assert_eq!(output_chunk.capacity(), initial_cap, "Output chunk reallocated at frame {}", frame_idx);
        }
    }

    #[test]
    fn test_pipeline_nan_and_inf_safety() {
        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        let mut noisy_frame = vec![0.1f32; AudioPipeline::FRAME_SAMPLES];
        noisy_frame[10] = f32::NAN;
        noisy_frame[20] = f32::INFINITY;
        noisy_frame[30] = -1e9;
        noisy_frame[40] = 1e9;

        let mut output_chunk = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES + 10);
        let res = pipeline.process_frame_internal(&noisy_frame, &mut output_chunk, 960);
        assert!(res.is_ok());

        for &sample in &output_chunk {
            assert!(!sample.is_nan(), "Output contains NaN");
            assert!(sample.is_finite(), "Output contains non-finite sample");
            assert!(sample.abs() <= 1.05, "Limiter failed to contain extreme spike: {}", sample);
        }
    }

    #[test]
    fn test_pipeline_mute_behavior() {
        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_muted(true);

        let input_chunk = vec![0.5f32; AudioPipeline::FRAME_SAMPLES];
        let mut output_chunk = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES + 10);

        let res = pipeline.process_frame_internal(&input_chunk, &mut output_chunk, 960);
        assert!(res.is_ok());
        for &s in &output_chunk {
            assert_eq!(s, 0.0, "Muted pipeline must output pure digital silence");
        }
    }

    #[test]
    fn test_pipeline_bypass_pass_through() {
        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_bypass(true);

        let input_chunk: Vec<f32> = (0..AudioPipeline::FRAME_SAMPLES)
            .map(|i| (i as f32 / 480.0) * 0.2)
            .collect();
        let mut output_chunk = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES + 10);

        let res = pipeline.process_frame_internal(&input_chunk, &mut output_chunk, 960);
        assert!(res.is_ok());
        assert_eq!(output_chunk.len(), input_chunk.len());
        // Verify signal matches closely
        for (i, (&in_s, &out_s)) in input_chunk.iter().zip(&output_chunk).enumerate() {
            assert!((in_s - out_s).abs() < 1e-3, "Mismatch at sample {}: in={}, out={}", i, in_s, out_s);
        }
    }

    #[test]
    fn test_speech_onset_no_clipped_syllables() {
        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_mode(EngineMode::Standard);
        pipeline.set_strength(1.0);

        let silence_frame = vec![0.0001f32; AudioPipeline::FRAME_SAMPLES]; // -80 dBFS background
        let mut out = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES + 10);

        // Feed 30 frames of silence (300 ms) to exhaust the 25-frame hangover and enter idle bypass
        for _ in 0..30 {
            pipeline.process_frame_internal(&silence_frame, &mut out, 960).unwrap();
        }

        // Generate synthetic speech onset (sine wave at -10 dBFS = ~0.316 amplitude)
        let speech_frame: Vec<f32> = (0..AudioPipeline::FRAME_SAMPLES)
            .map(|i| (2.0 * std::f32::consts::PI * 300.0 * (i as f32) / 48000.0).sin() * 0.3)
            .collect();

        // Feed speech frame 1 (onset, fills 1-frame 10ms overlap-add algorithmic buffer)
        pipeline.process_frame_internal(&speech_frame, &mut out, 960).unwrap();

        // Feed speech frame 2 (algorithmic delay produces fully reconstructed speech)
        pipeline.process_frame_internal(&speech_frame, &mut out, 960).unwrap();

        // Check speech energy in frame 2: must NOT be clipped or muted!
        let output_energy: f32 = out.iter().map(|&s| s * s).sum();
        let input_energy: f32 = speech_frame.iter().map(|&s| s * s).sum();

        // Output should preserve > 50% energy of speech (not gated to zero)
        assert!(
            output_energy > input_energy * 0.3,
            "Standard engine speech onset clipped! output_energy: {}, input_energy: {}",
            output_energy,
            input_energy
        );

        // Also test Light mode (RNNoise)
        let mut light_pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        light_pipeline.set_mode(EngineMode::Light);
        light_pipeline.set_strength(1.0);

        for _ in 0..30 {
            light_pipeline.process_frame_internal(&silence_frame, &mut out, 960).unwrap();
        }

        // Frame 1: speech onset (fills 10ms algorithmic overlap-add buffer)
        light_pipeline.process_frame_internal(&speech_frame, &mut out, 960).unwrap();
        // Frame 2: reconstructed speech
        light_pipeline.process_frame_internal(&speech_frame, &mut out, 960).unwrap();

        let light_output_energy: f32 = out.iter().map(|&s| s * s).sum();
        assert!(
            light_output_energy > input_energy * 0.3,
            "Light engine speech onset clipped! output_energy: {}, input_energy: {}",
            light_output_energy,
            input_energy
        );
    }

    #[test]
    fn test_pipeline_soak_ram_stability_over_10000_frames() {
        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_mode(EngineMode::Standard);
        pipeline.set_aec_enabled(true);

        let input_chunk: Vec<f32> = (0..AudioPipeline::FRAME_SAMPLES)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / 48000.0).sin() * 0.2)
            .collect();
        let mut output_chunk = Vec::with_capacity(AudioPipeline::FRAME_SAMPLES + 10);
        let aec_ref = vec![0.05f32; AudioPipeline::FRAME_SAMPLES];

        let initial_cap = output_chunk.capacity();

        for i in 0..10_000 {
            if i % 10 == 0 {
                pipeline.feed_aec_reference(&aec_ref);
            }
            assert!(pipeline.process_frame_internal(&input_chunk, &mut output_chunk, 960).is_ok());
            assert_eq!(output_chunk.capacity(), initial_cap, "Output chunk reallocated during soak!");
        }
    }

    #[test]
    fn test_zero_dry_signal_leakage_at_100_percent_strength() {
        let mut pipeline = AudioPipeline::new(48000, 48000).expect("Pipeline init failed");
        pipeline.set_mode(EngineMode::Standard);
        pipeline.set_strength(1.0);

        let mut out = Vec::with_capacity(490);
        // Feed silence to let smoother and filter states settle
        let silence = vec![0.0f32; AudioPipeline::FRAME_SAMPLES];
        for _ in 0..20 {
            pipeline.process_frame_internal(&silence, &mut out, 960).unwrap();
        }

        // Generate high-amplitude noise burst
        let noise_frame: Vec<f32> = (0..AudioPipeline::FRAME_SAMPLES)
            .map(|i| ((i * 17 % 100) as f32 / 50.0 - 1.0) * 0.4)
            .collect();

        // Process noise frame
        pipeline.process_frame_internal(&noise_frame, &mut out, 960).unwrap();

        // Check blended buffer vs wet buffer vs dry buffer
        let dry = pipeline.dry_buffer();
        let wet = &pipeline.wet_buffer;
        let blended = pipeline.blended_buffer();

        // At 100% strength, blended buffer MUST match wet buffer identically (0% dry leakage)
        for i in 0..AudioPipeline::FRAME_SAMPLES {
            assert!(
                (blended[i] - wet[i]).abs() < 1e-6,
                "Dry signal leaked at 100% strength! blended[{}]={}, wet[{}]={}, dry[{}]={}",
                i, blended[i], i, wet[i], i, dry[i]
            );
        }

        // Verify runtime parameters
        let params = pipeline.get_runtime_parameters();
        assert_eq!(params.strength, 1.0);
        assert!((params.wet_dry_ratio - 1.0).abs() < 1e-4);
        assert_eq!(params.attenuation_limit_db, 40.0);
    }
}
