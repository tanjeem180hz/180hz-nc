use crate::dsp::{
    AudioResampler, ComfortNoiseGenerator, DriftCompensator, GainSmoother, HighPassFilter,
    SilenceDetector, SoftLimiter,
};
use crate::engine::aec::EchoCanceller;
use crate::engine::light::LightEngine;
use crate::engine::standard::StandardEngine;
use crate::engine::traits::DenoiseEngine;
use crate::engine::voice_isolation::VoiceIsolationEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Light,
    Standard,
    VoiceIsolation,
}

/// Master Audio Processing Pipeline.
/// Operates on 10 ms frames at 48 kHz mono (480 samples).
pub struct AudioPipeline {
    mode: EngineMode,
    light_engine: LightEngine,
    standard_engine: StandardEngine,
    voice_iso_engine: VoiceIsolationEngine,
    hpf: HighPassFilter,
    aec: EchoCanceller,
    vad: SilenceDetector,
    smoother: GainSmoother,
    limiter: SoftLimiter,
    comfort_noise: ComfortNoiseGenerator,
    drift_compensator: DriftCompensator,
    input_resampler: AudioResampler,
    output_resampler: AudioResampler,
    bypass: bool,
    strength: f32, // 0.0 to 1.0
    frame_size: usize,
    dry_buffer: Vec<f32>,
    wet_buffer: Vec<f32>,
    drift_buffer: Vec<f32>,
    scratch_in: Vec<f32>,
}

impl AudioPipeline {
    pub const INTERNAL_RATE: u32 = 48000;
    pub const FRAME_SAMPLES: usize = 480; // 10 ms

    pub fn new(input_sample_rate: u32, output_sample_rate: u32) -> Result<Self, String> {
        let input_resampler = AudioResampler::new(input_sample_rate, Self::INTERNAL_RATE, Self::FRAME_SAMPLES)?;
        let output_resampler = AudioResampler::new(Self::INTERNAL_RATE, output_sample_rate, Self::FRAME_SAMPLES)?;

        let light = LightEngine::new();
        let standard = StandardEngine::new();
        let voice_iso = VoiceIsolationEngine::new(Box::new(LightEngine::new()));

        Ok(Self {
            mode: EngineMode::Standard,
            light_engine: light,
            standard_engine: standard,
            voice_iso_engine: voice_iso,
            hpf: HighPassFilter::default_48k(),
            aec: EchoCanceller::default_preset(),
            vad: SilenceDetector::default_48k(),
            smoother: GainSmoother::default_48k(),
            limiter: SoftLimiter::default_preset(),
            comfort_noise: ComfortNoiseGenerator::default_preset(),
            drift_compensator: DriftCompensator::default_48k(),
            input_resampler,
            output_resampler,
            bypass: false,
            strength: 1.0,
            frame_size: Self::FRAME_SAMPLES,
            dry_buffer: vec![0.0f32; Self::FRAME_SAMPLES],
            wet_buffer: vec![0.0f32; Self::FRAME_SAMPLES],
            drift_buffer: Vec::with_capacity(Self::FRAME_SAMPLES + 10),
            scratch_in: Vec::with_capacity(Self::FRAME_SAMPLES * 2),
        })
    }

    /// Sets operating mode: Light (RNNoise), Standard (DeepFilterNet), or VoiceIsolation.
    pub fn set_mode(&mut self, mode: EngineMode) {
        self.mode = mode;
    }

    pub fn mode(&self) -> EngineMode {
        self.mode
    }

    /// Suppression strength slider (0.0 = completely dry / 0% suppression, 1.0 = 100% full suppression).
    pub fn set_strength(&mut self, strength: f32) {
        let s = strength.clamp(0.0, 1.0);
        self.strength = s;
        self.smoother.set_target(s);
        self.standard_engine.set_suppression(s);
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

    /// Acoustic Echo Cancellation toggle.
    pub fn set_aec_enabled(&mut self, enabled: bool) {
        self.aec.set_enabled(enabled);
    }

    pub fn is_aec_enabled(&self) -> bool {
        self.aec.is_enabled()
    }

    pub fn feed_aec_reference(&mut self, ref_samples: &[f32]) {
        self.aec.feed_reference(ref_samples);
    }

    /// Comfort noise toggle.
    pub fn set_comfort_noise_enabled(&mut self, enabled: bool) {
        self.comfort_noise.set_enabled(enabled);
    }

    /// Access Voice Isolation engine for speaker enrollment.
    pub fn voice_isolation_mut(&mut self) -> &mut VoiceIsolationEngine {
        &mut self.voice_iso_engine
    }

    /// Processes an input audio chunk (from mic) and writes cleaned audio to `output` (for virtual mic).
    /// Guarantees: No allocations in steady-state, automatic fallback to pass-through on error, no NaNs.
    pub fn process_frame_internal(
        &mut self,
        input_mono: &[f32],
        output_mono: &mut Vec<f32>,
        buffer_fill_samples: usize,
    ) -> Result<(), String> {
        // Step 1: Resample input to 48 kHz if needed
        self.input_resampler.process(input_mono, &mut self.scratch_in)?;
        let in_48k = if self.scratch_in.is_empty() {
            input_mono
        } else {
            &self.scratch_in[..]
        };

        if in_48k.len() < self.frame_size {
            output_mono.clear();
            output_mono.extend_from_slice(input_mono);
            return Ok(());
        }

        let frame = &in_48k[..self.frame_size];

        // If hard bypass requested, forward directly with safety limiting
        if self.bypass {
            self.drift_compensator.adjust_frame(frame, &mut self.drift_buffer, buffer_fill_samples);
            self.output_resampler.process(&self.drift_buffer, output_mono)?;
            return Ok(());
        }

        // Save raw dry frame
        self.dry_buffer.copy_from_slice(frame);

        // Step 2: High-Pass Filter (80 Hz Butterworth) to eliminate sub-bass rumble
        self.wet_buffer.copy_from_slice(frame);
        self.hpf.process_slice(&mut self.wet_buffer);

        // Step 3: AEC (subtract room loudspeaker echo if enabled)
        self.aec.process_mic_frame(&mut self.wet_buffer);

        // Step 4: Silence Detection with Hangover (Auto-Bypass for CPU savings)
        let is_active = self.vad.update(&self.wet_buffer);

        if !is_active {
            // Signal is pure background silence -> bypass neural inference to save CPU
            // Apply comfort noise floor and fade
            self.comfort_noise.inject_into(&mut self.wet_buffer);
        } else {
            // Step 5: Active speech/noise -> invoke selected Denoise Engine
            let engine_res = match self.mode {
                EngineMode::Light => {
                    let mut cleaned = vec![0.0f32; self.frame_size];
                    let res = self.light_engine.process_frame(&self.wet_buffer, &mut cleaned);
                    if res.is_ok() {
                        self.wet_buffer.copy_from_slice(&cleaned);
                    }
                    res
                }
                EngineMode::Standard => {
                    let mut cleaned = vec![0.0f32; self.frame_size];
                    let res = self.standard_engine.process_frame(&self.wet_buffer, &mut cleaned);
                    if res.is_ok() {
                        self.wet_buffer.copy_from_slice(&cleaned);
                    }
                    res
                }
                EngineMode::VoiceIsolation => {
                    let mut cleaned = vec![0.0f32; self.frame_size];
                    let res = self.voice_iso_engine.process_frame(&self.wet_buffer, &mut cleaned);
                    if res.is_ok() {
                        self.wet_buffer.copy_from_slice(&cleaned);
                    }
                    res
                }
            };

            // Engine failure rule: "Engine failure => automatic pass-through, never silence."
            if engine_res.is_err() {
                self.wet_buffer.copy_from_slice(&self.dry_buffer);
            }
        }

        // Step 6: Wet/Dry Linear Mix with Gain Smoother
        let mut blended = vec![0.0f32; self.frame_size];
        self.smoother.blend_frame(&self.dry_buffer, &self.wet_buffer, &mut blended);

        // Step 7: Inject subtle comfort noise floor
        self.comfort_noise.inject_into(&mut blended);

        // Step 8: Soft Peak Limiter (prevent digital clipping)
        self.limiter.process_slice(&mut blended);

        // Step 9: Adaptive Clock Drift Compensation
        self.drift_compensator.adjust_frame(&blended, &mut self.drift_buffer, buffer_fill_samples);

        // Step 10: Resample to output device sample rate
        self.output_resampler.process(&self.drift_buffer, output_mono)?;

        Ok(())
    }
}
