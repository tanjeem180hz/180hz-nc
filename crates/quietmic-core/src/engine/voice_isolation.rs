use crate::engine::traits::DenoiseEngine;
use crate::dsp::GainSmoother;

/// Voice Isolation Engine (Experimental, Personalized Speech Filtering).
/// Enrolls the user's vocal characteristics (10-second enrollment) and suppresses
/// background voices (other people talking nearby) using spectral cosine similarity.
pub struct VoiceIsolationEngine {
    enrolled_embedding: Option<Vec<f32>>,
    current_similarity: f32,
    smoother: GainSmoother,
    frame_size: usize,
    base_engine: Box<dyn DenoiseEngine>,
}

impl VoiceIsolationEngine {
    pub const EMBEDDING_DIM: usize = 32;

    pub fn new(base_engine: Box<dyn DenoiseEngine>) -> Self {
        Self {
            enrolled_embedding: None,
            current_similarity: 1.0,
            smoother: GainSmoother::new(48000.0, 15.0, 80.0, 1.0),
            frame_size: 480,
            base_engine,
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
            let emb = Self::extract_timbre_features(frame);
            for i in 0..Self::EMBEDDING_DIM {
                avg_embedding[i] += emb[i];
            }
            count += 1;
            offset += self.frame_size;
        }

        if count == 0 {
            return Err("No valid frames for enrollment".to_string());
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

    /// Resets enrolled profile.
    pub fn clear_enrollment(&mut self) {
        self.enrolled_embedding = None;
    }

    /// Current cosine similarity of speaker to enrolled voiceprint (0.0 to 1.0).
    pub fn current_similarity(&self) -> f32 {
        self.current_similarity
    }

    /// Fast, deterministic spectral feature extractor (32-bin normalized energy spectrum).
    fn extract_timbre_features(frame: &[f32]) -> [f32; Self::EMBEDDING_DIM] {
        let mut features = [0.0f32; Self::EMBEDDING_DIM];
        let chunk_size = frame.len() / Self::EMBEDDING_DIM;

        for (i, bin) in features.iter_mut().enumerate() {
            let start = i * chunk_size;
            let end = (start + chunk_size).min(frame.len());
            let mut sum_sq = 0.0f32;
            for &s in &frame[start..end] {
                sum_sq += s * s;
            }
            *bin = (sum_sq / chunk_size.max(1) as f32).sqrt();
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

    fn process_frame(&mut self, in_frame: &[f32], out_frame: &mut [f32]) -> Result<(), String> {
        // First run base noise suppressor
        self.base_engine.process_frame(in_frame, out_frame)?;

        if let Some(ref enrolled) = self.enrolled_embedding {
            let current_feat = Self::extract_timbre_features(in_frame);
            let similarity = Self::cosine_similarity(enrolled, &current_feat);
            self.current_similarity = similarity;

            // Target gain: if speaker matches (similarity > 0.65), target = 1.0
            // If background talker (similarity < 0.4), suppress heavily (e.g. 0.05)
            let target_gain = if similarity >= 0.65 {
                1.0
            } else if similarity <= 0.35 {
                0.05
            } else {
                // Linear taper between 0.35 and 0.65
                (similarity - 0.35) / 0.30
            };

            self.smoother.set_target(target_gain);
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
}
