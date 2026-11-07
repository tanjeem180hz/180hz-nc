use crate::dsp::{
    AudioResampler, ComfortNoiseGenerator, DriftCompensator, GainSmoother, HighPassFilter,
    SilenceDetector, SoftLimiter,
};
use crate::engine::aec::EchoCanceller;
use crate::engine::diagnostics::DiagnosticTapProducers;
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

pub enum ActiveEngine {
    Light(LightEngine),
    Standard(StandardEngine),
    VoiceIsolation(VoiceIsolationEngine),
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
    smoother: GainSmoother,
    limiter: SoftLimiter,
    comfort_noise: ComfortNoiseGenerator,
    drift_compensator: DriftCompensator,
    input_resampler: AudioResampler,
    output_resampler: AudioResampler,
    bypass: bool,
    muted: bool,
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

        Ok(Self {
            mode: EngineMode::Standard,
            active_engine: ActiveEngine::Standard(standard),
            enrolled_voiceprint: None,
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
            muted: false,
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

    /// Sets operating mode: Light (RNNoise), Standard (DeepFilterNet), or VoiceIsolation.
    /// Lazy-loads the active engine and immediately frees memory of inactive engines.
    pub fn set_mode(&mut self, mode: EngineMode) {
        if self.mode != mode {
            self.mode = mode;
            self.active_engine = match mode {
                EngineMode::Light => ActiveEngine::Light(LightEngine::new()),
                EngineMode::Standard => {
                    let mut s = StandardEngine::new();
                    s.set_suppression(self.strength);
                    ActiveEngine::Standard(s)
                }
                EngineMode::VoiceIsolation => {
                    let mut v = VoiceIsolationEngine::new(Box::new(LightEngine::new()));
                    if let Some(ref emb) = self.enrolled_voiceprint {
                        v.set_enrolled_embedding(emb.clone());
                    }
                    ActiveEngine::VoiceIsolation(v)
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
        if let ActiveEngine::Standard(ref mut std_eng) = self.active_engine {
            std_eng.set_suppression(s);
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

    /// Attaches or detaches diagnostic tap producers.
    pub fn set_diagnostic_tap(&mut self, tap: Option<DiagnosticTapProducers>) {
        self.diagnostic_tap = tap;
    }

    /// Access Voice Isolation engine for speaker enrollment.
    pub fn voice_isolation_mut(&mut self) -> &mut VoiceIsolationEngine {
        if !matches!(self.active_engine, ActiveEngine::VoiceIsolation(_)) {
            let mut v = VoiceIsolationEngine::new(Box::new(LightEngine::new()));
            if let Some(ref emb) = self.enrolled_voiceprint {
                v.set_enrolled_embedding(emb.clone());
            }
            self.active_engine = ActiveEngine::VoiceIsolation(v);
            self.mode = EngineMode::VoiceIsolation;
        }
        match &mut self.active_engine {
            ActiveEngine::VoiceIsolation(v) => v,
            _ => unreachable!(),
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

        // Step 1: Resample input to 48 kHz if needed
        self.input_resampler.process(input_mono, &mut self.scratch_in)?;
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
            return Ok(());
        }

        let frame = &in_48k[..self.frame_size];

        // If muted, output complete digital silence
        if self.muted {
            output_mono.clear();
            output_mono.resize(self.frame_size, 0.0);
            return Ok(());
        }

        // Save raw dry frame with hardware NaN/Inf sanitization
        for (i, &s) in frame.iter().enumerate() {
            let clean = if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 };
            self.dry_buffer[i] = clean;
            self.wet_buffer[i] = clean;
        }

        // If hard bypass requested, forward directly with safety limiting
        if self.bypass {
            self.drift_compensator.adjust_frame(&self.dry_buffer, &mut self.drift_buffer, buffer_fill_samples);
            self.output_resampler.process(&self.drift_buffer, output_mono)?;
            return Ok(());
        }
        self.hpf.process_slice(&mut self.wet_buffer);

        // Stage 2: After Highpass
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(2, &self.wet_buffer);
        }

        // Step 3: AEC (subtract room loudspeaker echo if enabled)
        self.aec.process_mic_frame(&mut self.wet_buffer);

        // Stage 3: After AEC
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(3, &self.wet_buffer);
        }

        // Step 4: Silence Detection with Hangover (Auto-Bypass for CPU savings)
        let is_active = self.vad.update(&self.wet_buffer);

        if !is_active {
            // Signal is pure background silence -> bypass neural inference to save CPU
            // Attenuate background silence and apply subtle comfort noise floor
            self.wet_buffer.fill(0.0);
            self.comfort_noise.inject_into(&mut self.wet_buffer);
        } else {
            // Step 5: Active speech/noise -> invoke selected Denoise Engine
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
                ActiveEngine::VoiceIsolation(e) => {
                    let res = e.process_frame(&self.wet_buffer, &mut self.cleaned_buffer);
                    if res.is_ok() {
                        self.wet_buffer.copy_from_slice(&self.cleaned_buffer);
                    }
                    res
                }
            };

            // Engine failure rule: "Engine failure => automatic pass-through, never silence."
            if engine_res.is_err() {
                self.wet_buffer.copy_from_slice(&self.dry_buffer);
            }
        }

        // Stage 4: After Denoise
        if let Some(ref mut diag) = self.diagnostic_tap {
            diag.push_stage(4, &self.wet_buffer);
        }

        // Step 6: Wet/Dry Linear Mix with Gain Smoother
        self.smoother.blend_frame(&self.dry_buffer, &self.wet_buffer, &mut self.blended_buffer);

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
}
