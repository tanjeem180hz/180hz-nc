use std::f32::consts::PI;
use std::sync::Arc;
use realfft::{RealFftPlanner, RealToComplex};
use num_complex::Complex;
use crate::engine::traits::DenoiseEngine;
use crate::dsp::GainSmoother;

/// Voice Isolation Engine (Personalized Speaker Vocal Timbre Filter).
/// Enrolls the user's vocal tract characteristics (formants, F0, spectral distribution)
/// and suppresses background voices (secondary talkers, babble, cafe speech) using
/// 32-band spectral cosine similarity against the enrolled voiceprint.
pub struct VoiceIsolationEngine {
    enrolled_embedding: Option<Vec<f32>>,
    current_similarity: f32,
    smoother: GainSmoother,
    frame_size: usize,
    base_engine: Box<dyn DenoiseEngine>,
    r2c: Arc<dyn RealToComplex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    complex_spectrum: Vec<Complex<f32>>,
    time_scratch: Vec<f32>,
    window: Vec<f32>,
    band_ranges: [(usize, usize); Self::EMBEDDING_DIM],
}

impl VoiceIsolationEngine {
    pub const EMBEDDING_DIM: usize = 32;

    pub fn new(base_engine: Box<dyn DenoiseEngine>) -> Self {
        let frame_size = 480;
        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(frame_size);
        let fft_scratch = r2c.make_scratch_vec();
        let complex_spectrum = r2c.make_output_vec();
        let time_scratch = vec![0.0f32; frame_size];

        // Periodic Hann window
        let window: Vec<f32> = (0..frame_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / frame_size as f32).cos()))
            .collect();

        // 32 bands across bins 1..=96 (100 Hz to 9600 Hz)
        let mut band_ranges = [(1usize, 2usize); Self::EMBEDDING_DIM];
        // Bands 0..16: 1 bin each (100 Hz to 1600 Hz: F0 and formants F1/F2)
        for (b, range) in band_ranges.iter_mut().enumerate().take(16) {
            *range = (b + 1, b + 2);
        }
        // Bands 16..24: 2 bins each (1700 Hz to 3200 Hz: formants F2/F3)
        for b in 0..8 {
            let start = 17 + b * 2;
            band_ranges[16 + b] = (start, start + 2);
        }
        // Bands 24..28: 4 bins each (3300 Hz to 4800 Hz)
        for b in 0..4 {
            let start = 33 + b * 4;
            band_ranges[24 + b] = (start, start + 4);
        }
        // Bands 28..32: 12 bins each (4900 Hz to 9600 Hz)
        for b in 0..4 {
            let start = 49 + b * 12;
            band_ranges[28 + b] = (start, start + 12);
        }

        Self {
            enrolled_embedding: None,
            current_similarity: 1.0,
            smoother: GainSmoother::new(48000.0, 15.0, 80.0, 1.0),
            frame_size,
            base_engine,
            r2c,
            fft_scratch,
            complex_spectrum,
            time_scratch,
            window,
            band_ranges,
        }
    }

    /// Enrolls the speaker using recorded audio frames (typically ~10 seconds = 1000 frames).
    pub fn enroll(&mut self, audio_samples: &[f32]) -> Result<(), String> {
        if audio_samples.len() < 48000 {
            // Need at least 1 second minimum
            return Err("Enrollment recording too short (minimum 1 second required)".to_string());
        }

        let mut avg_embedding = vec![0.0f32; Self::EMBEDDING_DIM];
        let mut count = 0;

        let mut offset = 0;
        while offset + self.frame_size <= audio_samples.len() {
            let frame = &audio_samples[offset..offset + self.frame_size];
            let sum_sq: f32 = frame.iter().map(|&x| x * x).sum();
            let rms = (sum_sq / self.frame_size as f32).sqrt();

            // Accumulate active speech frames (skip silent gaps during enrollment)
            if rms > 0.005 {
                let emb = self.extract_timbre_features(frame);
                for i in 0..Self::EMBEDDING_DIM {
                    avg_embedding[i] += emb[i];
                }
                count += 1;
            }
            offset += self.frame_size;
        }

        if count == 0 {
            return Err("No active speech frames detected in enrollment audio".to_string());
        }

        for val in avg_embedding.iter_mut() {
            *val /= count as f32;
        }

        // L2 normalize
        Self::l2_normalize(&mut avg_embedding);
        self.enrolled_embedding = Some(avg_embedding);

        Ok(())
    }

    /// Whether the user has enrolled their voiceprint.
    pub fn is_enrolled(&self) -> bool {
        self.enrolled_embedding.is_some()
    }

    pub fn enrolled_embedding(&self) -> Option<&[f32]> {
        self.enrolled_embedding.as_deref()
    }

    pub fn set_enrolled_embedding(&mut self, embedding: Vec<f32>) {
        self.enrolled_embedding = Some(embedding);
    }

    /// Resets enrolled profile.
    pub fn clear_enrollment(&mut self) {
        self.enrolled_embedding = None;
    }

    /// Current cosine similarity of speaker to enrolled voiceprint (0.0 to 1.0).
    pub fn current_similarity(&self) -> f32 {
        self.current_similarity
    }

    /// Extract 32-band spectral magnitude timbre features.
    fn extract_timbre_features(&mut self, frame: &[f32]) -> [f32; Self::EMBEDDING_DIM] {
        let mut features = [0.0f32; Self::EMBEDDING_DIM];

        for (i, (&s, &w)) in frame.iter().zip(&self.window).enumerate() {
            self.time_scratch[i] = s * w;
        }

        if self.r2c.process_with_scratch(&mut self.time_scratch, &mut self.complex_spectrum, &mut self.fft_scratch).is_err() {
            return features;
        }

        for (b, &(start, end)) in self.band_ranges.iter().enumerate() {
            let mut sum_mag = 0.0f32;
            let bins = &self.complex_spectrum[start..end];
            for spec in bins {
                sum_mag += spec.norm();
            }
            features[b] = sum_mag / bins.len().max(1) as f32;
        }

        Self::l2_normalize(&mut features);
        features
    }

    fn l2_normalize(vec: &mut [f32]) {
        let norm_sq: f32 = vec.iter().map(|&x| x * x).sum();
        let norm = norm_sq.sqrt().max(1e-9);
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
        dot.clamp(0.0, 1.0)
    }
}

impl DenoiseEngine for VoiceIsolationEngine {
    fn name(&self) -> &'static str {
        "Voice Isolation (Personalized Speaker Filter)"
    }

    fn reset(&mut self) {
        self.base_engine.reset();
        self.current_similarity = 1.0;
        self.smoother.set_target(1.0);
    }

    fn speech_confidence(&self) -> f32 {
        self.base_engine.speech_confidence()
    }

    fn process_frame(&mut self, in_frame: &[f32], out_frame: &mut [f32]) -> Result<(), String> {
        // First run base noise suppressor
        self.base_engine.process_frame(in_frame, out_frame)?;

        if self.enrolled_embedding.is_some() {
            let sum_sq: f32 = in_frame.iter().map(|&x| x * x).sum();
            let frame_rms = (sum_sq / self.frame_size as f32).sqrt();

            if frame_rms > 0.003 {
                let current_feat = self.extract_timbre_features(in_frame);
                let enrolled = self.enrolled_embedding.as_deref().unwrap();
                let similarity = Self::cosine_similarity(enrolled, &current_feat);
                self.current_similarity = similarity;

                // Target gain: if speaker matches enrolled profile (similarity >= 0.55), pass cleanly
                // If background talker (similarity <= 0.30), suppress by ~22 dB (0.08)
                let target_gain = if similarity >= 0.55 {
                    1.0
                } else if similarity <= 0.30 {
                    0.08
                } else {
                    // Linear taper between 0.30 and 0.55
                    0.08 + 0.92 * ((similarity - 0.30) / 0.25)
                };

                self.smoother.set_target(target_gain);
            }

            self.smoother.apply_in_place(out_frame);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::light::LightEngine;

    #[test]
    fn test_enrollment_and_similarity() {
        let base = Box::new(LightEngine::new());
        let mut voice_iso = VoiceIsolationEngine::new(base);

        let synth_speech: Vec<f32> = (0..48000)
            .map(|i| (2.0 * std::f32::consts::PI * 220.0 * (i as f32) / 48000.0).sin())
            .collect();

        assert!(voice_iso.enroll(&synth_speech).is_ok());
        assert!(voice_iso.is_enrolled());

        let mut out = vec![0.0f32; 480];
        assert!(voice_iso.process_frame(&synth_speech[..480], &mut out).is_ok());
        assert!(voice_iso.current_similarity() > 0.85);
    }

    #[test]
    fn test_background_talker_rejection() {
        let base = Box::new(LightEngine::new());
        let mut voice_iso = VoiceIsolationEngine::new(base);

        // Enroll Speaker A (low pitch 150 Hz)
        let speaker_a: Vec<f32> = (0..48000)
            .map(|i| (2.0 * std::f32::consts::PI * 150.0 * (i as f32) / 48000.0).sin() * 0.4)
            .collect();
        assert!(voice_iso.enroll(&speaker_a).is_ok());

        // Process frames of Speaker B (higher pitch 950 Hz)
        let speaker_b: Vec<f32> = (0..4800)
            .map(|i| (2.0 * std::f32::consts::PI * 950.0 * (i as f32) / 48000.0).sin() * 0.4)
            .collect();

        let mut out = vec![0.0f32; 480];
        for f in 0..10 {
            let start = f * 480;
            let _ = voice_iso.process_frame(&speaker_b[start..start + 480], &mut out);
        }

        // Speaker B timbre does not match Speaker A voiceprint
        assert!(voice_iso.current_similarity() < 0.35, "Speaker B similarity ({:.3}) should be < 0.35", voice_iso.current_similarity());
    }
}
