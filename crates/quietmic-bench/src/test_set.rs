use std::f32::consts::PI;

#[allow(dead_code)]
pub struct AudioScenario {
    pub name: String,
    pub clean_speech: Vec<f32>,
    pub noisy_mixture: Vec<f32>,
    pub noise_only: Vec<f32>,
    pub target_snr_db: f32,
    pub sample_rate: u32,
}

pub struct TestSetGenerator;

impl TestSetGenerator {
    pub const SAMPLE_RATE: u32 = 48000;
    pub const DURATION_SECS: f32 = 3.0; // 3 seconds per benchmark clip

    /// Generates synthetic clean speech with natural phonetic formant modulation (vowels and fricatives).
    /// Emulates acoustic spectra of English and Bangla speech vowels (/a/, /i/, /u/, /o/).
    pub fn generate_clean_speech() -> Vec<f32> {
        let total_samples = (Self::DURATION_SECS * Self::SAMPLE_RATE as f32) as usize;
        let mut speech = vec![0.0f32; total_samples];

        // Fundamental frequencies (pitch ~ 130 Hz with natural intonation curve)
        for (i, s) in speech.iter_mut().enumerate() {
            let t = i as f32 / Self::SAMPLE_RATE as f32;
            let f0 = 130.0 + 15.0 * (2.0 * PI * 1.5 * t).sin(); // Pitch contour

            // Formants: F1=700Hz, F2=1220Hz, F3=2600Hz (/a/ vowel harmonics)
            let mut sample = 0.0f32;
            for h in 1..=20 {
                let freq = f0 * h as f32;
                if freq > 8000.0 {
                    break;
                }
                // Resonance weighting
                let f1_dist = ((freq - 700.0) / 150.0).powi(2);
                let f2_dist = ((freq - 1220.0) / 200.0).powi(2);
                let gain = (-f1_dist).exp() * 1.0 + (-f2_dist).exp() * 0.6 + 0.1 / (h as f32);
                sample += gain * (2.0 * PI * freq * t).sin();
            }

            // Syllabic envelope modulation (speaking cadence: 4 syllables per second)
            let envelope = 0.5 * (1.0 + (2.0 * PI * 3.5 * t).sin()).powf(2.0);
            *s = sample * envelope * 0.15;
        }

        speech
    }

    /// Generates realistic mechanical keyboard click transients.
    pub fn generate_keyboard_clicks() -> Vec<f32> {
        let total_samples = (Self::DURATION_SECS * Self::SAMPLE_RATE as f32) as usize;
        let mut noise = vec![0.0f32; total_samples];

        // Place clicks every ~200-300 ms
        let click_intervals = [0.2, 0.45, 0.65, 0.9, 1.2, 1.4, 1.7, 1.95, 2.3, 2.6];
        for &t_click in &click_intervals {
            let center_idx = (t_click * Self::SAMPLE_RATE as f32) as usize;
            let click_len = 240; // 5 ms transient burst
            for j in 0..click_len {
                let idx = center_idx + j;
                if idx < total_samples {
                    let decay = (-10.0 * (j as f32 / click_len as f32)).exp();
                    let osc = (2.0 * PI * 3200.0 * (j as f32 / Self::SAMPLE_RATE as f32)).sin();
                    noise[idx] += osc * decay * 0.4;
                }
            }
        }

        noise
    }

    /// Generates steady low-frequency AC / HVAC / fan hum (60 Hz / 120 Hz + wind turbulence).
    pub fn generate_fan_ac_noise() -> Vec<f32> {
        let total_samples = (Self::DURATION_SECS * Self::SAMPLE_RATE as f32) as usize;
        let mut noise = vec![0.0f32; total_samples];

        let mut rng_state: u32 = 0x5EED_F00D;
        let mut lpf = 0.0f32;

        for (i, val) in noise.iter_mut().enumerate() {
            let t = i as f32 / Self::SAMPLE_RATE as f32;
            // Mains hum & fan motor harmonics
            let hum = 0.3 * (2.0 * PI * 60.0 * t).sin()
                    + 0.15 * (2.0 * PI * 120.0 * t).sin()
                    + 0.08 * (2.0 * PI * 240.0 * t).sin();

            // Air turbulence pink noise
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 17;
            rng_state ^= rng_state << 5;
            let white = (rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0;
            lpf += 0.1 * (white - lpf);

            *val = hum * 0.2 + lpf * 0.15;
        }

        noise
    }

    /// Generates cocktail party cafe babble (unintelligible background chatter).
    pub fn generate_cafe_babble() -> Vec<f32> {
        let total_samples = (Self::DURATION_SECS * Self::SAMPLE_RATE as f32) as usize;
        let mut noise = vec![0.0f32; total_samples];

        // Superimpose 5 asynchronous speech-like voices
        let voice_pitches = [105.0, 160.0, 210.0, 250.0, 180.0];
        for (v, &f0) in voice_pitches.iter().enumerate() {
            let speed = 2.0 + v as f32 * 0.5;
            for (i, sample_val) in noise.iter_mut().enumerate() {
                let t = i as f32 / Self::SAMPLE_RATE as f32;
                let sample = (2.0 * PI * f0 * t).sin() + 0.5 * (2.0 * PI * f0 * 2.0 * t).sin();
                let mod_env = 0.5 * (1.0 + (2.0 * PI * speed * t + v as f32).sin());
                *sample_val += sample * mod_env * 0.05;
            }
        }

        noise
    }

    /// Generates distinct background secondary talker (different pitch 240 Hz female vs 130 Hz male target).
    pub fn generate_background_talker() -> Vec<f32> {
        let total_samples = (Self::DURATION_SECS * Self::SAMPLE_RATE as f32) as usize;
        let mut noise = vec![0.0f32; total_samples];

        for (i, val) in noise.iter_mut().enumerate() {
            let t = i as f32 / Self::SAMPLE_RATE as f32;
            let f0 = 240.0 + 20.0 * (2.0 * PI * 2.0 * t).sin();
            let voice = (2.0 * PI * f0 * t).sin() + 0.6 * (2.0 * PI * f0 * 2.0 * t).sin();
            let env = 0.5 * (1.0 + (2.0 * PI * 4.0 * t).sin()).powf(2.0);
            *val = voice * env * 0.2;
        }

        noise
    }

    /// Mixes clean speech with noise to achieve exact target SNR in dB.
    pub fn mix_at_snr(speech: &[f32], noise: &[f32], target_snr_db: f32) -> (Vec<f32>, Vec<f32>) {
        let len = speech.len().min(noise.len());
        let speech_slice = &speech[..len];
        let noise_slice = &noise[..len];

        let s_energy: f32 = speech_slice.iter().map(|&x| x * x).sum();
        let n_energy: f32 = noise_slice.iter().map(|&x| x * x).sum();

        if n_energy < 1e-10 {
            return (speech_slice.to_vec(), vec![0.0f32; len]);
        }

        let target_ratio = 10.0f32.powf(-target_snr_db / 10.0);
        let scale = ((s_energy * target_ratio) / n_energy).sqrt();

        let scaled_noise: Vec<f32> = noise_slice.iter().map(|&x| x * scale).collect();
        let mixture: Vec<f32> = speech_slice
            .iter()
            .zip(scaled_noise.iter())
            .map(|(&s, &n)| s + n)
            .collect();

        (mixture, scaled_noise)
    }

    /// Creates full standard benchmark test suite across scenarios and SNRs (-5 dB to 10 dB).
    pub fn create_test_suite() -> Vec<AudioScenario> {
        let clean = Self::generate_clean_speech();
        let snrs = [-5.0, 0.0, 5.0, 10.0];
        let mut suite = Vec::new();

        let noise_types = [
            ("Keyboard_Clicks", Self::generate_keyboard_clicks()),
            ("Fan_AC_Hum", Self::generate_fan_ac_noise()),
            ("Cafe_Babble", Self::generate_cafe_babble()),
            ("Background_Talker", Self::generate_background_talker()),
        ];

        for (noise_name, noise) in noise_types {
            for &snr in &snrs {
                let (mixture, scaled_noise) = Self::mix_at_snr(&clean, &noise, snr);
                suite.push(AudioScenario {
                    name: format!("{}_{:+}dB_SNR", noise_name, snr as i32),
                    clean_speech: clean.clone(),
                    noisy_mixture: mixture,
                    noise_only: scaled_noise,
                    target_snr_db: snr,
                    sample_rate: Self::SAMPLE_RATE,
                });
            }
        }

        suite
    }
}
