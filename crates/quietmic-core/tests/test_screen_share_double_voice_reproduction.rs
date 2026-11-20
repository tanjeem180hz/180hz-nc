use quietmic_core::engine::aec::EchoCanceller;
use std::f32::consts::PI;
use std::path::Path;

fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut writer) = hound::WavWriter::create(path, spec) {
        for &s in samples {
            let clamped = s.clamp(-1.0, 1.0);
            let val = (clamped * 32767.0) as i16;
            let _ = writer.write_sample(val);
        }
        let _ = writer.finalize();
    }
}

/// Synthesizes realistic human speech with harmonic formants (F0 ~ 150 Hz, voiced pitch pulses)
fn synthesize_speech(num_samples: usize, f0: f32) -> Vec<f32> {
    let mut out = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / 48000.0;
        let pitch = f0 + 25.0 * (2.0 * PI * 2.0 * t).sin();
        let envelope = (2.0 * PI * 2.5 * t).sin().max(0.0).powf(1.6);
        let mut sample = 0.0f32;
        sample += (2.0 * PI * pitch * t).sin() * 0.40;
        sample += (2.0 * PI * (pitch * 2.0) * t).sin() * 0.25;
        sample += (2.0 * PI * (pitch * 3.0) * t).sin() * 0.15;
        sample += (2.0 * PI * 1800.0 * t).sin() * 0.08;
        sample += (2.0 * PI * 2500.0 * t).sin() * 0.05;
        out.push(sample * envelope * 0.45);
    }
    out
}

#[test]
fn test_screen_share_voice_echo_reproduction_and_elimination() {
    println!("\n=================================================================");
    println!(">>> RUNNING SCREEN SHARE VOICE ECHO & DOUBLE VOICE REPRODUCTION <<<");
    println!("=================================================================");

    const FRAME_SIZE: usize = 480;
    const NUM_FRAMES: usize = 500; // 5.0 seconds
    const TOTAL_SAMPLES: usize = NUM_FRAMES * FRAME_SIZE;

    // 1. Far-end dialogue (e.g. YouTube video, game voices, or Discord call participants)
    let far_end_voice = synthesize_speech(TOTAL_SAMPLES, 140.0);

    // 2. Near-end user speaking into microphone (from 2.5s to 4.5s)
    let mut near_end_user = vec![0.0f32; TOTAL_SAMPLES];
    let user_speech = synthesize_speech(2 * 48000, 185.0);
    near_end_user[2 * 48000..4 * 48000].copy_from_slice(&user_speech);

    // 3. Acoustic loop: sound from speakers travels into physical microphone
    // Realistic room acoustic delay: 35 ms = 1680 samples
    let acoustic_delay = 1680;
    let mut mic_input = vec![0.0f32; TOTAL_SAMPLES];
    for i in 0..TOTAL_SAMPLES {
        let mut echo = 0.0f32;
        if i >= acoustic_delay {
            // Direct acoustic path (attenuated by distance from speakers to mic)
            echo += far_end_voice[i - acoustic_delay] * 0.35;
            // Early desk/monitor reflections
            if i >= acoustic_delay + 48 {
                echo += far_end_voice[i - acoustic_delay - 48] * 0.15;
            }
            if i >= acoustic_delay + 144 {
                echo += far_end_voice[i - acoustic_delay - 144] * 0.08;
            }
        }
        mic_input[i] = echo + near_end_user[i];
    }

    // --- Scenario 1: Without AEC / Unconverged AEC (The Reported Defect) ---
    // In this defective scenario, the microphone transmits the leaked speaker sound.
    // Viewers of screen share receive: Desktop Audio (Track 1) + Mic Input (Track 2).
    // Viewers hear Track 1 + Track 2: The voice arrives twice (delayed by 35 ms)!
    let mut defective_viewer_mix = Vec::with_capacity(TOTAL_SAMPLES);
    for i in 0..TOTAL_SAMPLES {
        // Track 1 (Desktop) + Track 2 (Leaked Mic)
        defective_viewer_mix.push(far_end_voice[i] + mic_input[i]);
    }

    // Measure double-voice correlation in defective mix:
    // The delay between the two voices causes comb filtering and double voice.
    let leak_rms_before = (mic_input[48000..2 * 48000].iter().map(|&x| x * x).sum::<f32>() / 48000.0).sqrt();
    println!("Defective (No AEC / Bypassed): Leaked Speaker Audio RMS into Mic = {:.5}", leak_rms_before);

    // --- Scenario 2: With 180hz nc Acoustic Echo Cancellation Active ---
    let mut canceller = EchoCanceller::default_preset();
    canceller.set_enabled(true);

    let mut cleaned_mic = Vec::with_capacity(TOTAL_SAMPLES);
    let mut cleaned_viewer_mix = Vec::with_capacity(TOTAL_SAMPLES);

    for f in 0..NUM_FRAMES {
        let start = f * FRAME_SIZE;
        let end = start + FRAME_SIZE;
        let ref_frame = &far_end_voice[start..end];
        let mut mic_frame = mic_input[start..end].to_vec();

        // Feed reference loopback audio and process mic frame
        canceller.feed_reference(ref_frame);
        canceller.process_mic_frame(&mut mic_frame);

        for (i, &clean_s) in mic_frame.iter().enumerate() {
            cleaned_mic.push(clean_s);
            // In cleaned screen share, viewer receives: Desktop Audio (Track 1) + Cleaned Mic (Track 2)
            cleaned_viewer_mix.push(far_end_voice[start + i] + clean_s);
        }
    }

    // Measure echo suppression during single-talk far-end (1.0s to 2.0s):
    let eval_range = 48000..2 * 48000;
    let raw_leak_rms = (mic_input[eval_range.clone()].iter().map(|&x| x * x).sum::<f32>() / 48000.0).sqrt();
    let cleaned_leak_rms = (cleaned_mic[eval_range.clone()].iter().map(|&x| x * x).sum::<f32>() / 48000.0).sqrt();
    let erle_db = 20.0 * (raw_leak_rms / (cleaned_leak_rms + 1e-6)).log10();

    println!("Cleaned with AEC: Raw Leak RMS = {:.5}, Cleaned Leak RMS = {:.5}, ERLE = {:.2} dB", raw_leak_rms, cleaned_leak_rms, erle_db);
    println!("AEC Telemetry: Locked = {}, Delay = {:.1} ms ({} samples), Bypass Reason = {}",
        canceller.is_delay_locked(), canceller.bulk_delay_ms(), canceller.bulk_delay_samples(), canceller.bypass_reason_code());

    // Measure user's voice preservation during double-talk (3.0s to 4.0s):
    let dt_range = 3 * 48000..4 * 48000;
    let expected_user_rms = (near_end_user[dt_range.clone()].iter().map(|&x| x * x).sum::<f32>() / 48000.0).sqrt();
    let actual_user_rms = (cleaned_mic[dt_range.clone()].iter().map(|&x| x * x).sum::<f32>() / 48000.0).sqrt();
    let user_delta_db = (actual_user_rms / (expected_user_rms + 1e-6)).log10() * 20.0;
    println!("Double-talk User Voice Preservation: Expected RMS = {:.4}, Output RMS = {:.4}, Delta = {:.2} dB",
        expected_user_rms, actual_user_rms, user_delta_db);

    // Export WAVs for audit
    let base_dirs = [Path::new("ab_samples"), Path::new("../../ab_samples")];
    for base in &base_dirs {
        write_wav(&base.join("screenshare_double_voice_defective.wav"), &defective_viewer_mix, 48000);
        write_wav(&base.join("screenshare_double_voice_cleaned.wav"), &cleaned_viewer_mix, 48000);
        write_wav(&base.join("screenshare_mic_raw_leak.wav"), &mic_input, 48000);
        write_wav(&base.join("screenshare_mic_cleaned.wav"), &cleaned_mic, 48000);
    }

    assert!(erle_db >= 20.0, "ERLE must be >= 20.0 dB to eliminate screen share voice echo, got {:.2} dB", erle_db);
    assert!(user_delta_db.abs() < 1.5, "User's voice must not be attenuated or distorted during double talk, delta = {:.2} dB", user_delta_db);
}


#[test]
fn test_screen_share_silent_gap_and_room_reverberation() {
    println!("\n=================================================================");
    println!(">>> RUNNING SCREEN SHARE SILENT GAP (WASAPI NO PACKETS) TEST <<<");
    println!("=================================================================");

    const FRAME_SIZE: usize = 480;
    const NUM_FRAMES: usize = 600; // 6.0 seconds
    const TOTAL_SAMPLES: usize = NUM_FRAMES * FRAME_SIZE;

    // Segment 1 (0.0s to 1.5s): Audio plays
    // Segment 2 (1.5s to 3.5s): SILENT GAP (2.0 seconds: WASAPI loopback delivers 0 packets!)
    // Segment 3 (3.5s to 6.0s): Audio resumes
    let mut far_end_voice = vec![0.0f32; TOTAL_SAMPLES];
    let speech1 = synthesize_speech(72000, 150.0);
    far_end_voice[0..72000].copy_from_slice(&speech1);

    let speech2 = synthesize_speech(120000, 160.0);
    far_end_voice[168000..288000].copy_from_slice(&speech2);

    let acoustic_delay = 1440; // 30 ms
    let mut mic_input = vec![0.0f32; TOTAL_SAMPLES];
    for i in 0..TOTAL_SAMPLES {
        if i >= acoustic_delay {
            let mut echo = far_end_voice[i - acoustic_delay] * 0.35;
            // Room reverberation taps at 10 ms, 18 ms, 25 ms
            if i >= acoustic_delay + 480 {
                echo += far_end_voice[i - acoustic_delay - 480] * 0.15;
            }
            if i >= acoustic_delay + 864 {
                echo += far_end_voice[i - acoustic_delay - 864] * 0.08;
            }
            if i >= acoustic_delay + 1200 {
                echo += far_end_voice[i - acoustic_delay - 1200] * 0.04;
            }
            mic_input[i] = echo;
        }
    }

    let mut canceller = EchoCanceller::default_preset();
    canceller.set_enabled(true);

    let mut cleaned_mic = Vec::with_capacity(TOTAL_SAMPLES);

    for f in 0..NUM_FRAMES {
        let start = f * FRAME_SIZE;
        let end = start + FRAME_SIZE;
        let ref_frame = &far_end_voice[start..end];
        let mut mic_frame = mic_input[start..end].to_vec();

        // Simulate WASAPI loopback:
        // During 1.5s to 3.5s (frames 150 to 350), WASAPI loopback delivers NO PACKETS!
        let ref_peak = ref_frame.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        if ref_peak > 1e-5 {
            canceller.feed_reference(ref_frame);
        } else {
            // WASAPI delivers NO PACKETS during silence!
            // When no packets arrive, what happens if we feed nothing?
        }

        canceller.process_mic_frame(&mut mic_frame);
        cleaned_mic.extend_from_slice(&mic_frame);
    }

    // Measure suppression after speech resumes (4.0s to 5.5s):
    let resume_range = 4 * 48000..55 * 4800; // 4.0s to 5.5s
    let raw_resume_rms = (mic_input[resume_range.clone()].iter().map(|&x| x * x).sum::<f32>() / (1.5 * 48000.0)).sqrt();
    let clean_resume_rms = (cleaned_mic[resume_range.clone()].iter().map(|&x| x * x).sum::<f32>() / (1.5 * 48000.0)).sqrt();
    let erle_resume_db = 20.0 * (raw_resume_rms / (clean_resume_rms + 1e-6)).log10();

    println!("Silent Gap Test: Raw Resumed RMS = {:.5}, Cleaned Resumed RMS = {:.5}, ERLE = {:.2} dB",
        raw_resume_rms, clean_resume_rms, erle_resume_db);
    println!("AEC Telemetry after gap: Locked = {}, Delay = {:.1} ms ({} samples), Bypass Reason = {}",
        canceller.is_delay_locked(), canceller.bulk_delay_ms(), canceller.bulk_delay_samples(), canceller.bypass_reason_code());

    assert!(erle_resume_db >= 20.0, "AEC must immediately suppress echo after 2-second silent gap with >= 20 dB ERLE, got {:.2} dB", erle_resume_db);
}


#[test]
fn test_screen_share_double_talk_from_start() {
    println!("\n=================================================================");
    println!(">>> RUNNING SCREEN SHARE DOUBLE TALK FROM START TEST <<<");
    println!("=================================================================");

    const FRAME_SIZE: usize = 480;
    const NUM_FRAMES: usize = 300; // 3.0 seconds
    const TOTAL_SAMPLES: usize = NUM_FRAMES * FRAME_SIZE;

    // Both far-end caller AND near-end user speak simultaneously from the start
    let far_end_voice = synthesize_speech(TOTAL_SAMPLES, 135.0);
    let near_end_user = synthesize_speech(TOTAL_SAMPLES, 210.0);

    let acoustic_delay = 1920; // 40 ms
    let mut mic_input = vec![0.0f32; TOTAL_SAMPLES];
    for i in 0..TOTAL_SAMPLES {
        let echo = if i >= acoustic_delay { far_end_voice[i - acoustic_delay] * 0.35 } else { 0.0 };
        mic_input[i] = echo + near_end_user[i];
    }

    let mut canceller = EchoCanceller::default_preset();
    canceller.set_enabled(true);

    let mut cleaned_mic = Vec::with_capacity(TOTAL_SAMPLES);

    for f in 0..NUM_FRAMES {
        let start = f * FRAME_SIZE;
        let end = start + FRAME_SIZE;
        let ref_frame = &far_end_voice[start..end];
        let mut mic_frame = mic_input[start..end].to_vec();

        canceller.feed_reference(ref_frame);
        canceller.process_mic_frame(&mut mic_frame);
        cleaned_mic.extend_from_slice(&mic_frame);
    }

    // Near-end user speech level preservation check (1.0s to 3.0s)
    let eval_range = 48000..TOTAL_SAMPLES;
    let expected_rms = (near_end_user[eval_range.clone()].iter().map(|&x| x * x).sum::<f32>() / (eval_range.len() as f32)).sqrt();
    let actual_rms = (cleaned_mic[eval_range.clone()].iter().map(|&x| x * x).sum::<f32>() / (eval_range.len() as f32)).sqrt();
    let delta_db = (actual_rms / (expected_rms + 1e-6)).log10() * 20.0;

    println!("Double Talk from Start: Expected User RMS = {:.4}, Output RMS = {:.4}, Delta = {:.2} dB",
        expected_rms, actual_rms, delta_db);
    println!("AEC Telemetry: Locked = {}, Delay = {:.1} ms ({} samples), Bypass Reason = {}, dt = {}",
        canceller.is_delay_locked(), canceller.bulk_delay_ms(), canceller.bulk_delay_samples(), canceller.bypass_reason_code(), canceller.is_double_talk());

    assert!(delta_db.abs() < 2.0, "User voice must not be destroyed during initial double-talk: delta = {:.2} dB", delta_db);
}
