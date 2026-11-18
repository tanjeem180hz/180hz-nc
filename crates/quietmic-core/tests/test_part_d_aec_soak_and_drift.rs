use quietmic_core::engine::aec::EchoCanceller;
use std::f32::consts::PI;

#[test]
fn test_aec_soak_and_clock_drift_stability() {
    println!("\n=======================================================");
    println!(">>> RUNNING AEC SOAK, CLOCK DRIFT & JITTER TEST <<<");
    println!("=======================================================");

    let mut canceller = EchoCanceller::default_preset();
    canceller.set_enabled(true);

    const FRAME_SIZE: usize = 480;
    const NUM_SOAK_FRAMES: usize = 6000; // 60 seconds of continuous accelerated processing

    let mut delay_shift_samples = 480; // 10 ms initial delay
    canceller.set_bulk_delay_samples(delay_shift_samples);

    let mut max_output_amp = 0.0f32;
    let mut total_output_energy = 0.0f32;
    let mut high_freq_chatter_energy = 0.0f32;

    for frame_idx in 0..NUM_SOAK_FRAMES {
        let t = frame_idx as f32 * 0.01;

        // Dynamic delay shifts every 500 frames (simulating OS buffer changes or Bluetooth drift)
        if frame_idx % 500 == 0 {
            delay_shift_samples = (delay_shift_samples + 96) % 1920; // 2 ms increments up to 40 ms
            canceller.set_bulk_delay_samples(delay_shift_samples);
        }

        // Occasional device restart simulation every 2000 frames
        if frame_idx % 2000 == 0 && frame_idx > 0 {
            canceller.reset();
            canceller.set_bulk_delay_samples(delay_shift_samples);
        }

        // Clock drift simulation: Reference stream produces variable chunk size (475 to 485 samples)
        let ref_chunk_len = if frame_idx % 2 == 0 { 482 } else { 478 };
        let mut ref_chunk = Vec::with_capacity(ref_chunk_len);
        for i in 0..ref_chunk_len {
            let sample_t = t + (i as f32 / 48000.0);
            let s = (2.0 * PI * 220.0 * sample_t).sin() * 0.35 + (2.0 * PI * 440.0 * sample_t).sin() * 0.15;
            ref_chunk.push(s);
        }

        // Feed reference into bounded FIFO
        canceller.feed_reference(&ref_chunk);

        // Microphone input receives room reflection
        let mut mic_frame = vec![0.0f32; FRAME_SIZE];
        for (i, m) in mic_frame.iter_mut().enumerate() {
            let sample_t = t + (i as f32 / 48000.0);
            *m = (2.0 * PI * 220.0 * sample_t).sin() * 0.12;
        }

        // Process frame
        canceller.process_mic_frame(&mut mic_frame);

        // Verify output integrity per sample
        for i in 0..FRAME_SIZE {
            let s = mic_frame[i];
            assert!(!s.is_nan(), "NaN detected at frame {} sample {}", frame_idx, i);
            assert!(!s.is_infinite(), "Inf detected at frame {} sample {}", frame_idx, i);
            let abs_s = s.abs();
            if abs_s > max_output_amp {
                max_output_amp = abs_s;
            }
            total_output_energy += s * s;
            if i > 0 {
                let diff = s - mic_frame[i - 1];
                high_freq_chatter_energy += diff * diff;
            }
        }
    }

    let total_samples = (NUM_SOAK_FRAMES * FRAME_SIZE) as f32;
    let avg_rms = (total_output_energy / total_samples).sqrt();
    let chatter_rms = (high_freq_chatter_energy / total_samples).sqrt();

    println!("Soak Completed: {} frames processed.", NUM_SOAK_FRAMES);
    println!("Max Output Amplitude: {:.4}", max_output_amp);
    println!("Average Output RMS: {:.5}", avg_rms);
    println!("High-Freq Chatter RMS: {:.5}", chatter_rms);
    println!("Final ERLE: {:.2} dB", canceller.last_erle_db());

    assert!(max_output_amp < 1.05, "Output exploded beyond 1.05! max = {:.4}", max_output_amp);
    assert!(chatter_rms < 0.005, "AEC exhibited high-frequency ringing / chatter! chatter_rms = {:.5}", chatter_rms);
}
