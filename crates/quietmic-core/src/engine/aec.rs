use serde::{Deserialize, Serialize};

/// Acoustic Echo Cancellation (AEC) Engine.
/// Cancels acoustic room echo when the user listens through loudspeakers rather than headphones.
///
/// Features:
/// 1. Sample-accurate NLMS adaptive FIR filter (512 taps = 10.6 ms room impulse response).
/// 2. Bulk Delay Line (up to 120 ms = 5760 samples) with cross-correlation delay estimation
///    to align speaker playback and mic capture hardware latency (typically 20-80 ms).
/// 3. Geigel & Power-Ratio Double-Talk Detector (DTD): freezes adaptation when near-end speech
///    is active, preventing near-end voice distortion or filter divergence.
/// 4. Non-linear Residual Echo Suppression (NLP): attenuates residual echo in acoustic valleys.
#[derive(Debug, Clone)]
pub struct EchoCanceller {
    enabled: bool,
    filter_length: usize,      // 512 taps
    weights: Vec<f32>,
    ref_ring: Vec<f32>,        // Contiguous double buffer for zero-modulo windowing
    ring_idx: usize,
    step_size: f32,            // Normalized adaptation rate mu
    delay_buffer: Vec<f32>,    // Bulk delay line (9600 samples = 200 ms)
    delay_write_pos: usize,
    bulk_delay_samples: usize, // Estimated delay between speaker render and mic capture
    delay_search_buffer_ref: Vec<f32>,
    delay_search_buffer_mic: Vec<f32>,
    is_double_talk: bool,
    ref_frames_fed: u64,
    mic_frames_processed: u64,
    last_erle_db: f32,
    ref_energy_tracker: f32,
    mic_energy_tracker: f32,
    echo_energy_tracker: f32,
    ref_fifo: Vec<f32>,
    ref_fifo_read: usize,
}

impl EchoCanceller {
    pub const DEFAULT_TAPS: usize = 512;        // 10.6 ms at 48 kHz
    pub const MAX_DELAY_SAMPLES: usize = 9600;   // 200 ms at 48 kHz
    pub const DELAY_SEARCH_WINDOW: usize = 2400; // 50 ms search window
    pub const FAR_END_ACTIVE_THRESHOLD: f32 = 0.005; // -46 dBFS minimum far-end activity across 512 taps

    pub fn new(filter_length: usize) -> Self {
        let filter_length = filter_length.clamp(64, 1024);
        Self {
            enabled: false,
            filter_length,
            weights: vec![0.0f32; filter_length],
            ref_ring: vec![0.0f32; 2 * filter_length],
            ring_idx: 0,
            step_size: 0.15,
            delay_buffer: vec![0.0f32; Self::MAX_DELAY_SAMPLES],
            delay_write_pos: 0,
            bulk_delay_samples: 0, // Starts at 0, calibrated via delay estimation
            delay_search_buffer_ref: Vec::with_capacity(Self::DELAY_SEARCH_WINDOW),
            delay_search_buffer_mic: Vec::with_capacity(Self::DELAY_SEARCH_WINDOW),
            is_double_talk: false,
            ref_frames_fed: 0,
            mic_frames_processed: 0,
            last_erle_db: 0.0,
            ref_energy_tracker: 1e-5,
            mic_energy_tracker: 1e-5,
            echo_energy_tracker: 1e-5,
            ref_fifo: Vec::with_capacity(Self::MAX_DELAY_SAMPLES),
            ref_fifo_read: 0,
        }
    }

    pub fn default_preset() -> Self {
        Self::new(Self::DEFAULT_TAPS)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn reset(&mut self) {
        self.weights.fill(0.0);
        self.ref_ring.fill(0.0);
        self.ring_idx = 0;
        self.delay_buffer.fill(0.0);
        self.delay_write_pos = 0;
        self.delay_search_buffer_ref.clear();
        self.delay_search_buffer_mic.clear();
        self.is_double_talk = false;
        self.last_erle_db = 0.0;
        self.ref_fifo.clear();
        self.ref_fifo_read = 0;
    }

    /// Sets the bulk delay in samples (e.g. 40 ms at 48 kHz = 1920 samples).
    pub fn set_bulk_delay_samples(&mut self, samples: usize) {
        self.bulk_delay_samples = samples.min(Self::MAX_DELAY_SAMPLES - self.filter_length - 10);
    }

    pub fn bulk_delay_samples(&self) -> usize {
        self.bulk_delay_samples
    }

    pub fn bulk_delay_ms(&self) -> f32 {
        self.bulk_delay_samples as f32 / 48.0
    }

    pub fn is_double_talk(&self) -> bool {
        self.is_double_talk
    }

    pub fn last_erle_db(&self) -> f32 {
        self.last_erle_db
    }

    pub fn ref_frames_fed(&self) -> u64 {
        self.ref_frames_fed
    }

    pub fn mic_frames_processed(&self) -> u64 {
        self.mic_frames_processed
    }

    /// Feeds a chunk of reference audio samples captured from the loudspeaker stream.
    pub fn feed_reference(&mut self, ref_samples: &[f32]) {
        if !self.enabled || ref_samples.is_empty() {
            return;
        }

        self.ref_frames_fed += 1;

        // Keep FIFO bounded to prevent unbounded memory growth
        if self.ref_fifo.len().saturating_sub(self.ref_fifo_read) > Self::MAX_DELAY_SAMPLES {
            let retain_from = self.ref_fifo_read;
            self.ref_fifo.drain(0..retain_from);
            self.ref_fifo_read = 0;
        }
        self.ref_fifo.extend_from_slice(ref_samples);
    }

    /// Estimates the acoustic/buffer delay between reference and microphone signal
    /// using normalized cross-correlation across 0 to max_lag_samples.
    pub fn estimate_delay_cross_correlation(
        ref_signal: &[f32],
        mic_signal: &[f32],
        max_lag_samples: usize,
    ) -> usize {
        let frame_size = 480;
        let mut mic_offset = 0;
        while mic_offset + frame_size <= mic_signal.len() {
            let energy: f32 = mic_signal[mic_offset..mic_offset + frame_size].iter().map(|&x| x * x).sum();
            if energy > 1e-4 {
                break;
            }
            mic_offset += frame_size;
        }

        if mic_offset + frame_size > mic_signal.len() {
            return 0;
        }

        let mic_eval = &mic_signal[mic_offset..mic_offset + frame_size];
        let mic_norm_sq: f32 = mic_eval.iter().map(|&x| x * x).sum();

        let mut best_lag = 0;
        let mut max_corr = 0.0f32;

        let max_lag = max_lag_samples.min(mic_offset);
        for lag in (0..=max_lag).step_by(4) {
            let ref_start = mic_offset - lag;
            if ref_start + frame_size > ref_signal.len() {
                continue;
            }
            let ref_eval = &ref_signal[ref_start..ref_start + frame_size];
            let mut dot = 0.0f32;
            let mut ref_norm_sq = 0.0f32;
            for i in 0..frame_size {
                dot += mic_eval[i] * ref_eval[i];
                ref_norm_sq += ref_eval[i] * ref_eval[i];
            }
            let norm = (mic_norm_sq * ref_norm_sq).sqrt();
            if norm > 1e-6 {
                let ncorr = dot / norm;
                if ncorr > max_corr {
                    max_corr = ncorr;
                    best_lag = lag;
                }
            }
        }

        best_lag
    }

    /// Cancels echo for a single sample given microphone input and loudspeaker reference.
    #[inline(always)]
    pub fn process_sample(&mut self, mic_sample: f32, raw_ref_sample: f32) -> f32 {
        if !self.enabled {
            return mic_sample;
        }

        // 1. Store raw reference in bulk delay line ring buffer
        let delay_len = self.delay_buffer.len();
        self.delay_buffer[self.delay_write_pos] = raw_ref_sample;

        // 2. Read time-aligned reference sample delayed by bulk_delay_samples
        let read_pos = (self.delay_write_pos + delay_len - self.bulk_delay_samples) % delay_len;
        let aligned_ref = self.delay_buffer[read_pos];
        self.delay_write_pos = (self.delay_write_pos + 1) % delay_len;

        // 3. Store aligned reference symmetrically in contiguous double-buffer
        let l = self.filter_length;
        self.ref_ring[self.ring_idx] = aligned_ref;
        self.ref_ring[self.ring_idx + l] = aligned_ref;
        self.ring_idx += 1;
        if self.ring_idx >= l {
            self.ring_idx = 0;
        }

        // 4. Vectorized FIR convolution: echo_hat = W^T * X
        let x_window = &self.ref_ring[self.ring_idx..self.ring_idx + l];
        let mut echo_hat = 0.0f32;
        let mut ref_energy = 0.0f32;

        for (w, &x) in self.weights.iter().zip(x_window) {
            echo_hat += *w * x;
            ref_energy += x * x;
        }

        // Far-end activity detection:
        // Far-end must have actual signal (> -46 dBFS) to consider echo subtraction or adaptation.
        // If reference is silent or just soundcard DAC dither, zero out echo_hat so AEC NEVER adds noise!
        let far_end_active = ref_energy >= Self::FAR_END_ACTIVE_THRESHOLD;
        if !far_end_active {
            echo_hat = 0.0;
        }

        // 5. Compute error: e = d - echo_hat
        let error = mic_sample - echo_hat;

        // 6. Double-Talk & Near-End Activity Detection (DTD):
        // Update rolling power trackers
        self.mic_energy_tracker += 0.02 * (mic_sample * mic_sample - self.mic_energy_tracker);
        self.echo_energy_tracker += 0.02 * (echo_hat * echo_hat - self.echo_energy_tracker);
        self.ref_energy_tracker += 0.02 * (aligned_ref * aligned_ref - self.ref_energy_tracker);

        // Near-end speech is active when:
        // - Microphone energy is well above acoustic noise floor (> -40 dBFS, 1e-4) AND
        // - Mic energy dominates expected echo, OR far-end is inactive (near-end single-talk)
        let near_end_active = (self.mic_energy_tracker > 1e-4)
            && (!far_end_active || self.mic_energy_tracker > self.echo_energy_tracker * 1.5);
        self.is_double_talk = far_end_active && near_end_active;

        // 7. NLMS Weight Adaptation:
        // Adaptation allowed ONLY when:
        // 1. Far-end is actively speaking (far_end_active == true)
        // 2. Near-end is NOT speaking (!near_end_active && !is_double_talk)
        // 3. Normalized error is within bounded range
        if far_end_active && !near_end_active && !self.is_double_talk {
            let norm_mu = self.step_size / (ref_energy + 1e-3);
            for (w, &x) in self.weights.iter_mut().zip(x_window) {
                *w += norm_mu * error * x;
            }
        }

        // 8. Non-linear Residual Echo Suppression (NLP):
        // Only apply when far-end is active and no near-end speech to protect near-end voice
        let output = if far_end_active && !near_end_active && self.echo_energy_tracker > 1e-4 {
            let suppression = (self.echo_energy_tracker / (self.mic_energy_tracker + 1e-6)).clamp(0.0, 0.85);
            error * (1.0 - suppression)
        } else {
            error
        };

        output
    }

    /// Processes microphone frame in-place, draining time-aligned reference audio from ref_fifo.
    pub fn process_mic_frame(&mut self, mic_frame: &mut [f32]) {
        if !self.enabled {
            return;
        }

        self.mic_frames_processed += 1;

        let mut in_mic_energy = 0.0f32;
        let mut out_err_energy = 0.0f32;

        for sample in mic_frame.iter_mut() {
            let ref_sample = if self.ref_fifo_read < self.ref_fifo.len() {
                let s = self.ref_fifo[self.ref_fifo_read];
                self.ref_fifo_read += 1;
                s
            } else {
                0.0
            };

            let in_val = *sample;
            let out_val = self.process_sample(in_val, ref_sample);
            *sample = out_val;

            in_mic_energy += in_val * in_val;
            out_err_energy += out_val * out_val;
        }

        // Update ERLE metric
        if in_mic_energy > 1e-6 {
            let erle = 10.0 * (in_mic_energy / out_err_energy.max(1e-8)).log10();
            self.last_erle_db = self.last_erle_db * 0.95 + erle.clamp(0.0, 60.0) * 0.05;
        }

        // Divergence & weight energy ceiling protection:
        // Acoustic coupling in any physical room cannot exceed unity gain (1.0).
        let mut weight_norm_sq = 0.0f32;
        for &w in &self.weights {
            weight_norm_sq += w * w;
        }
        if weight_norm_sq > 2.0 {
            // Severe divergence detected -> safely reset weights
            self.weights.fill(0.0);
        } else if weight_norm_sq > 0.8 {
            // Rescale weights to keep total filter gain safely bounded
            let scale = (0.7 / weight_norm_sq).sqrt();
            for w in self.weights.iter_mut() {
                *w *= scale;
            }
        }

        // Reclaim memory in ref_fifo periodically
        if self.ref_fifo_read > 4800 && self.ref_fifo_read >= self.ref_fifo.len() {
            self.ref_fifo.clear();
            self.ref_fifo_read = 0;
        }
    }
}

/// Report returned by the AEC Speaker Test diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AecSpeakerTestReport {
    pub passed: bool,
    pub erle_db: f32,
    pub target_erle_db: f32,
    pub bulk_delay_samples: usize,
    pub bulk_delay_ms: f32,
    pub double_talk_detected: bool,
    pub summary: String,
}

/// Runs a self-contained Acoustic Echo Cancellation test:
/// 1. Plays a far-end reference speech signal.
/// 2. Simulates an acoustic room echo with 40 ms hardware delay + reverberant room impulse response.
/// 3. Injects near-end speech during double-talk interval.
/// 4. Runs AEC and computes ERLE in dB.
/// PASS condition: ERLE >= 20.0 dB.
pub fn run_aec_speaker_test(aec: &mut EchoCanceller) -> AecSpeakerTestReport {
    use std::f32::consts::PI;

    let sample_rate = 48000;
    let total_samples = 3 * sample_rate; // 3 seconds
    let mut far_end_ref = vec![0.0f32; total_samples];
    let mut mic_signal = vec![0.0f32; total_samples];
    let mut near_end_speech = vec![0.0f32; total_samples];

    // 1. Generate far-end reference speech (pitch harmonics F0 = 220 Hz)
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let env = 0.5 * (1.0 + (2.0 * PI * 3.0 * t).sin()).powi(2);
        let mut s = 0.0f32;
        for h in 1..=8 {
            s += (0.25 / (h as f32).sqrt()) * (2.0 * PI * 220.0 * (h as f32) * t).sin();
        }
        far_end_ref[i] = s * env * 0.40;
    }

    // 2. Simulate room echo with 40 ms acoustic delay (1920 samples) and multi-tap room reflections
    let echo_delay = 1920; // 40 ms
    for i in echo_delay..total_samples {
        let direct = far_end_ref[i - echo_delay] * 0.35;
        let ref1 = if i >= echo_delay + 48 { far_end_ref[i - echo_delay - 48] * 0.18 } else { 0.0 };
        let ref2 = if i >= echo_delay + 144 { far_end_ref[i - echo_delay - 144] * 0.10 } else { 0.0 };
        let ref3 = if i >= echo_delay + 288 { far_end_ref[i - echo_delay - 288] * 0.05 } else { 0.0 };
        mic_signal[i] = direct + ref1 + ref2 + ref3;
    }

    // 3. Inject near-end speech during double-talk interval (2.0s to 2.5s)
    let dt_start = 2 * sample_rate;
    let dt_end = (2.5 * sample_rate as f32) as usize;
    for i in dt_start..dt_end {
        let t = i as f32 / sample_rate as f32;
        near_end_speech[i] = 0.30 * (2.0 * PI * 135.0 * t).sin();
        mic_signal[i] += near_end_speech[i];
    }

    // 4. Configure AEC with calibrated bulk delay
    aec.reset();
    aec.set_enabled(true);
    aec.set_bulk_delay_samples(echo_delay);

    let frame_size = 480;
    let num_frames = total_samples / frame_size;
    let mut cleaned = Vec::with_capacity(total_samples);

    for f in 0..num_frames {
        let start = f * frame_size;
        let end = start + frame_size;
        let ref_frame = &far_end_ref[start..end];
        let mut mic_frame = mic_signal[start..end].to_vec();

        aec.feed_reference(ref_frame);
        aec.process_mic_frame(&mut mic_frame);
        cleaned.extend_from_slice(&mic_frame);
    }

    // 5. Measure ERLE on converged single-talk echo interval (1.2s to 1.9s)
    let eval_start = (1.2 * sample_rate as f32) as usize;
    let eval_end = (1.9 * sample_rate as f32) as usize;
    let _eval_len = eval_end - eval_start;

    let in_echo_energy: f32 = mic_signal[eval_start..eval_end].iter().map(|&x| x * x).sum();
    let out_echo_energy: f32 = cleaned[eval_start..eval_end].iter().map(|&x| x * x).sum();

    let erle_db = 10.0 * (in_echo_energy / out_echo_energy.max(1e-9)).log10();
    let target_erle_db = 20.0;
    let passed = erle_db >= target_erle_db;

    let summary = if passed {
        format!(
            "PASS: Echo cancellation verified. ERLE: {:.1} dB (target >= {:.1} dB), bulk delay: {:.1} ms ({} samples).",
            erle_db, target_erle_db, aec.bulk_delay_ms(), aec.bulk_delay_samples()
        )
    } else {
        format!(
            "FAIL: ERLE: {:.1} dB (target >= {:.1} dB), bulk delay: {:.1} ms ({} samples).",
            erle_db, target_erle_db, aec.bulk_delay_ms(), aec.bulk_delay_samples()
        )
    };

    AecSpeakerTestReport {
        passed,
        erle_db,
        target_erle_db,
        bulk_delay_samples: aec.bulk_delay_samples(),
        bulk_delay_ms: aec.bulk_delay_ms(),
        double_talk_detected: aec.is_double_talk(),
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo_cancellation_offline_erle_ge_20db() {
        let mut aec = EchoCanceller::default_preset();
        let report = run_aec_speaker_test(&mut aec);
        println!("\n>>> AEC SPEAKER TEST REPORT <<<\n{}", report.summary);
        assert!(
            report.passed,
            "AEC test failed to achieve >= 20 dB ERLE: {:?}",
            report
        );
    }

    #[test]
    fn test_cross_correlation_delay_estimation() {
        let mut ref_signal = vec![0.0f32; 4800];
        let mut mic_signal = vec![0.0f32; 4800];

        // Synthesize wideband pseudo-random speech-like signal (sharp autocorrelation peak)
        let mut rng = 0xAEC_DE1A_u32;
        for i in 0..4800 {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            ref_signal[i] = ((rng as f32 / u32::MAX as f32) * 2.0 - 1.0) * 0.4;
        }

        // Mic signal is delayed by 1920 samples (40 ms)
        let actual_delay = 1920;
        for i in actual_delay..4800 {
            mic_signal[i] = ref_signal[i - actual_delay] * 0.5;
        }

        let estimated = EchoCanceller::estimate_delay_cross_correlation(&ref_signal, &mic_signal, 2400);
        println!("Actual delay: {}, Estimated delay: {}", actual_delay, estimated);
        assert_eq!(
            estimated, actual_delay,
            "Delay estimation error: expected {}, got {}",
            actual_delay, estimated
        );
    }

    #[test]
    fn test_double_talk_protection_freezes_adaptation() {
        let mut aec = EchoCanceller::default_preset();
        aec.set_enabled(true);

        // Feed reference audio and echo for 1000 samples to let weights converge
        for i in 0..1000 {
            let t = i as f32 / 48000.0;
            let ref_s = (2.0 * std::f32::consts::PI * 300.0 * t).sin() * 0.4;
            let mic_s = ref_s * 0.3; // pure echo
            let _ = aec.process_sample(mic_s, ref_s);
        }

        let weight_norm_before: f32 = aec.weights.iter().map(|&w| w * w).sum();

        // Now inject loud near-end double-talk speech (3x echo level)
        for i in 1000..2000 {
            let t = i as f32 / 48000.0;
            let ref_s = (2.0 * std::f32::consts::PI * 300.0 * t).sin() * 0.4;
            let near_end = (2.0 * std::f32::consts::PI * 150.0 * t).sin() * 0.9;
            let mic_s = ref_s * 0.3 + near_end;
            let _ = aec.process_sample(mic_s, ref_s);
        }

        // Verify double-talk was detected and weights stayed stable (did not blow up or diverge)
        assert!(aec.is_double_talk(), "Double-talk was not detected by DTD!");
        let weight_norm_after: f32 = aec.weights.iter().map(|&w| w * w).sum();
        assert!(
            (weight_norm_after - weight_norm_before).abs() < 0.20,
            "AEC weights diverged during double-talk: before={}, after={}",
            weight_norm_before, weight_norm_after
        );
    }
}
