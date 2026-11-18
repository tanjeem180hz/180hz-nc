use hound::{WavSpec, WavWriter};
use quietmic_core::engine::pipeline::{AudioPipeline, EngineMode};
use std::f32::consts::PI;
use std::fs;
use std::path::Path;

fn write_wav_file(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create wav writer at {:?}: {}", path, e))?;
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0) as i16;
        writer.write_sample(sample_i16)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }
    writer.finalize().map_err(|e| format!("Failed to finalize wav: {}", e))?;
    Ok(())
}

fn generate_speech(t_start: f32, num_samples: usize, amplitude: f32) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = t_start + (i as f32 / 48000.0);
        let f0 = 150.0;
        let v1 = (2.0 * PI * f0 * t).sin() * 0.45;
        let v2 = (2.0 * PI * (f0 * 2.0) * t).sin() * 0.28;
        let v3 = (2.0 * PI * (f0 * 3.0) * t).sin() * 0.18;
        let v4 = (2.0 * PI * (f0 * 5.0) * t).sin() * 0.09;
        out.push((v1 + v2 + v3 + v4) * amplitude);
    }
    out
}

fn process_whole_stream(
    pipeline: &mut AudioPipeline,
    input_samples: &[f32],
    feed_aec: bool,
    aec_ref: Option<&[f32]>,
) -> Vec<f32> {
    let frame_size = 480;
    let mut out = Vec::with_capacity(input_samples.len());
    let mut scratch = Vec::with_capacity(490);
    let mut offset = 0;
    let silent_ref = vec![0.0f32; frame_size];

    while offset + frame_size <= input_samples.len() {
        let frame = &input_samples[offset..offset + frame_size];
        if feed_aec {
            if let Some(r) = aec_ref {
                if offset + frame_size <= r.len() {
                    pipeline.feed_aec_reference(&r[offset..offset + frame_size]);
                } else {
                    pipeline.feed_aec_reference(&silent_ref);
                }
            } else {
                pipeline.feed_aec_reference(&silent_ref);
            }
        }
        let _ = pipeline.process_frame_internal(frame, &mut scratch, 960);
        out.extend_from_slice(&scratch);
        offset += frame_size;
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = Path::new("ab_samples");
    if !out_dir.exists() {
        fs::create_dir_all(out_dir)?;
    }

    println!("=======================================================");
    println!(">>> GENERATING OBJECTIVE A/B AUDIT SAMPLES IN ab_samples/ <<<");
    println!("=======================================================");

    const SAMPLE_RATE: u32 = 48000;
    const TOTAL_SECONDS: usize = 3;
    let total_samples = TOTAL_SECONDS * SAMPLE_RATE as usize;

    // 1. Loud Speech (-3 dBFS) without cracking or dropouts
    println!("Generating: loud_speech_raw.wav vs loud_speech_cleaned.wav...");
    let mut loud_raw = Vec::with_capacity(total_samples);
    for frame_idx in 0..(total_samples / 480) {
        let t = frame_idx as f32 * 0.01;
        let spk = generate_speech(t, 480, 0.70); // Peak ~ 0.70 (-3 dBFS)
        loud_raw.extend_from_slice(&spk);
    }
    let mut pipe_loud = AudioPipeline::new(SAMPLE_RATE, SAMPLE_RATE).unwrap();
    pipe_loud.set_mode(EngineMode::Light);
    pipe_loud.set_strength(1.0);
    let loud_cleaned = process_whole_stream(&mut pipe_loud, &loud_raw, false, None);

    write_wav_file(&out_dir.join("loud_speech_raw.wav"), &loud_raw, SAMPLE_RATE).unwrap();
    write_wav_file(&out_dir.join("loud_speech_cleaned.wav"), &loud_cleaned, SAMPLE_RATE).unwrap();

    // 2. Whisper Speech (-35 dBFS) preserved by Auto-Sensitivity
    println!("Generating: whisper_raw.wav vs whisper_cleaned.wav...");
    let mut whisper_raw = Vec::with_capacity(total_samples);
    let mut rng = 13579u32;
    for frame_idx in 0..(total_samples / 480) {
        let t = frame_idx as f32 * 0.01;
        let spk = generate_speech(t, 480, 0.018); // ~ -35 dBFS
        for &s in &spk {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let room_noise = (rng as f32 / u32::MAX as f32 - 0.5) * 0.002; // -60 dBFS floor
            whisper_raw.push(s + room_noise);
        }
    }
    let mut pipe_whisper = AudioPipeline::new(SAMPLE_RATE, SAMPLE_RATE).unwrap();
    pipe_whisper.set_mode(EngineMode::Light);
    pipe_whisper.set_auto_sensitivity(true);
    let whisper_cleaned = process_whole_stream(&mut pipe_whisper, &whisper_raw, false, None);

    write_wav_file(&out_dir.join("whisper_raw.wav"), &whisper_raw, SAMPLE_RATE).unwrap();
    write_wav_file(&out_dir.join("whisper_cleaned.wav"), &whisper_cleaned, SAMPLE_RATE).unwrap();

    // 3. AEC ON with Silent Reference (Zero Added Noise)
    println!("Generating: aec_on_silence_raw.wav vs aec_on_silence_cleaned.wav...");
    let mut aec_silence_raw = Vec::with_capacity(total_samples);
    for _ in 0..total_samples {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let room_hiss = (rng as f32 / u32::MAX as f32 - 0.5) * 0.001; // -66 dBFS
        aec_silence_raw.push(room_hiss);
    }
    let mut pipe_aec = AudioPipeline::new(SAMPLE_RATE, SAMPLE_RATE).unwrap();
    pipe_aec.set_mode(EngineMode::None); // Test AEC isolation alone
    pipe_aec.set_aec_enabled(true);
    let aec_silence_cleaned = process_whole_stream(&mut pipe_aec, &aec_silence_raw, true, None);

    write_wav_file(&out_dir.join("aec_on_silence_raw.wav"), &aec_silence_raw, SAMPLE_RATE).unwrap();
    write_wav_file(&out_dir.join("aec_on_silence_cleaned.wav"), &aec_silence_cleaned, SAMPLE_RATE).unwrap();

    // 4. Keyboard Clicks & Fan Hum mixture
    println!("Generating: keyboard_fan_raw.wav vs cleaned versions...");
    let mut kbd_fan_raw = Vec::with_capacity(total_samples);
    for frame_idx in 0..(total_samples / 480) {
        let t = frame_idx as f32 * 0.01;
        // Speech during first 1.5 seconds, then pause
        let spk = if frame_idx < 150 {
            generate_speech(t, 480, 0.35)
        } else {
            vec![0.0f32; 480]
        };

        for (i, &s) in spk.iter().enumerate() {
            let sample_t = (frame_idx * 480 + i) as f32 / 48000.0;
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let white = (rng as f32 / u32::MAX as f32 - 0.5) * 0.04;
            let fan = (2.0 * PI * 60.0 * sample_t).sin() * 0.06 + (2.0 * PI * 120.0 * sample_t).sin() * 0.03;
            let click = if (frame_idx % 25 == 0) && i < 150 {
                let env = (-8.0 * (i as f32 / 150.0)).exp();
                (2.0 * PI * 3500.0 * sample_t).sin() * env * 0.40
            } else {
                0.0
            };
            kbd_fan_raw.push(s + fan + white + click);
        }
    }

    let mut pipe_krisp = AudioPipeline::new(SAMPLE_RATE, SAMPLE_RATE).unwrap();
    pipe_krisp.set_mode(EngineMode::Light);
    pipe_krisp.set_strength(1.0);
    let kbd_cleaned_krisp = process_whole_stream(&mut pipe_krisp, &kbd_fan_raw, false, None);

    let mut pipe_std = AudioPipeline::new(SAMPLE_RATE, SAMPLE_RATE).unwrap();
    pipe_std.set_mode(EngineMode::Standard);
    pipe_std.set_strength(1.0);
    let kbd_cleaned_std = process_whole_stream(&mut pipe_std, &kbd_fan_raw, false, None);

    let mut pipe_max = AudioPipeline::new(SAMPLE_RATE, SAMPLE_RATE).unwrap();
    pipe_max.set_mode(EngineMode::MaxQuality);
    pipe_max.set_strength(1.0);
    let kbd_cleaned_max = process_whole_stream(&mut pipe_max, &kbd_fan_raw, false, None);

    write_wav_file(&out_dir.join("keyboard_fan_raw.wav"), &kbd_fan_raw, SAMPLE_RATE).unwrap();
    write_wav_file(&out_dir.join("keyboard_fan_cleaned_krisp.wav"), &kbd_cleaned_krisp, SAMPLE_RATE).unwrap();
    write_wav_file(&out_dir.join("keyboard_fan_cleaned_standard.wav"), &kbd_cleaned_std, SAMPLE_RATE).unwrap();
    write_wav_file(&out_dir.join("keyboard_fan_cleaned_maxquality.wav"), &kbd_cleaned_max, SAMPLE_RATE).unwrap();

    println!(">>> ALL A/B AUDIT WAV SAMPLES SUCCESSFULLY EXPORTED TO ab_samples/ <<<");
    Ok(())
}
