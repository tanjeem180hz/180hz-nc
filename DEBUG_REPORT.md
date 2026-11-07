# QuietMic Diagnostic & Debugging Report: Root-Cause Analysis and Fixes

**Repository**: [tanjeem180hz/180hz-nc](https://github.com/tanjeem180hz/180hz-nc)  
**Branch**: `main`  
**Date**: November 2026  
**Status**: Step 1 & Step 2 Diagnostics Complete — Active Bug Fix Phase

---

## 1. Symptom Statement & Initial Observations
- **Reported Symptom**: Noise cancellation in QuietMic is not suppressing background noise properly in real-world usage. Specifically, keyboard clicks and fan noise remain audible in Light mode, voice sound quality has high-frequency buzz or distortion in Standard mode, and syllables are occasionally cut off during low-volume speech.
- **Goal**: Identify the exact root causes using empirical evidence and quantitative measurements across all 8 pipeline stages. Fix every identified bug with zero regressions, maintaining $\ge 15$ dB stationary noise reduction, $< 1$ dB speech distortion, zero memory allocations in the audio thread, and push all commits to `origin main`.

---

## 2. Observable Pipeline Architecture (8 Diagnostic Taps)

To eliminate guesswork, an 8-stage SPSC lock-free diagnostic tap dumper was engineered into `AudioPipeline`. Real-time audio callbacks dump uncompressed 48 kHz 16-bit mono WAV files and compute per-stage RMS and peak dBFS via a non-blocking background thread:

1. `01_raw_input.wav`: Raw capture buffer from microphone.
2. `02_after_resample.wav`: After sinc resampling to 48 kHz.
3. `03_after_highpass.wav`: After 80 Hz 2nd-order Butterworth HPF (DC and rumble removal).
4. `04_after_aec.wav`: After NLMS acoustic echo cancellation.
5. `05_after_denoise.wav`: Immediately after the AI/DSP denoise engine (`Light`, `Standard`, or `VoiceIsolation`).
6. `06_after_wet_dry_mix.wav`: After linear/smooth crossfade between dry and wet buffers.
7. `07_after_postprocess_limiter.wav`: After comfort noise injection and soft limiter.
8. `08_final_output.wav`: After clock drift compensation and output device resampling.

### Diagnostic Sweep Baseline Measurement (Pre-Fix)
Evaluated on a standardized 3-second test signal combining stationary fan hum (60 Hz + 120 Hz + white noise floor), vowel speech harmonics (300 Hz, 600 Hz, 1200 Hz at $t = 1.0..2.0$s), and keyboard transient clicks (3.5 kHz at $t = 0.5$s, $1.5$s, $2.5$s):

#### Pre-Fix Diagnostic Sweep — Standard Mode
| Stage Name | Noise RMS | Speech RMS | SNR Diff | Attenuation vs HPF |
| :--- | :---: | :---: | :---: | :---: |
| `01_raw_input` | -30.1 dB | -14.3 dB | +15.8 dB | 0.0 dB |
| `02_after_resample` | -30.1 dB | -14.3 dB | +15.8 dB | 0.0 dB |
| `03_after_highpass` | -32.0 dB | -14.3 dB | +17.7 dB | Baseline (-1.9 dB HPF) |
| `04_after_aec` | -32.0 dB | -14.3 dB | +17.7 dB | 0.0 dB |
| `05_after_denoise` | -34.4 dB | -14.4 dB | +20.0 dB | **-2.4 dB (FAIL - expected $\ge 15$ dB)** |
| `06_after_wet_dry_mix` | -34.4 dB | -14.4 dB | +20.0 dB | -2.4 dB |
| `07_after_postprocess_limiter` | -34.4 dB | -14.4 dB | +20.0 dB | -2.4 dB |
| `08_final_output` | -34.4 dB | -14.4 dB | +20.0 dB | -2.4 dB |

#### Pre-Fix Diagnostic Sweep — Light Mode (RNNoise)
| Stage Name | Noise RMS | Speech RMS | SNR Diff | Attenuation vs HPF |
| :--- | :---: | :---: | :---: | :---: |
| `01_raw_input` | -30.1 dB | -14.3 dB | +15.8 dB | 0.0 dB |
| `02_after_resample` | -30.1 dB | -14.3 dB | +15.8 dB | 0.0 dB |
| `03_after_highpass` | -32.0 dB | -14.3 dB | +17.7 dB | Baseline (-1.9 dB HPF) |
| `04_after_aec` | -32.0 dB | -14.3 dB | +17.7 dB | 0.0 dB |
| `05_after_denoise` | -32.0 dB | -14.3 dB | +17.6 dB | **0.0 dB (CRITICAL FAIL - ZERO noise reduction)** |
| `06_after_wet_dry_mix` | -32.0 dB | -14.3 dB | +17.6 dB | 0.0 dB |
| `07_after_postprocess_limiter` | -32.0 dB | -14.3 dB | +17.6 dB | 0.0 dB |
| `08_final_output` | -32.0 dB | -14.3 dB | +17.6 dB | 0.0 dB |

---

## 3. Systematic Investigation of All 9 Causes

### Cause 1: Sample Scaling
- **Status**: **FOUND BUG (CRITICAL)**
- **Hypothesis**: RNNoise (`nnnoiseless`) assumes 16-bit float amplitude range ($[-32768.0, 32767.0]$), whereas the QuietMic pipeline operates in normalized float range ($[-1.0, 1.0]$).
- **Test**: Created unit test `test_nnnoiseless_scaling_behavior` feeding identical sine waves in $[-1.0, 1.0]$ vs scaled $[-32768.0, 32767.0]$ into `nnnoiseless::DenoiseState`.
- **Result**:
  - Input $[-1.0, 1.0]$: $\text{VAD} = 0.0037$ (99.6% silence detection). The recurrent network treated the input as digital floor noise under 1 LSB and applied no filtering.
  - Input $[-32768.0, 32767.0]$: $\text{VAD} = 0.4067$ (valid speech detection). Filter active.
- **Fix**: In `LightEngine::process_frame`, multiply `in_frame` by `32768.0` into a pre-allocated scratch buffer, call `self.state.process_frame`, and multiply `out_frame` by `1.0 / 32768.0`.

---

### Cause 2: Frame Size and Alignment / StandardEngine Frequency Domain Processing
- **Status**: **FOUND BUG (CRITICAL)**
- **Hypothesis**: `StandardEngine` was designed as an STFT frequency-subband Wiener filter, but line-by-line inspection revealed that NO Fourier transform was being computed.
- **Evidence**: In `StandardEngine::process_frame` lines 97–159, the 960-sample time-domain analysis window was divided into 64 slices of 15 samples each. It calculated time-domain energy over 15 samples and directly multiplied those time-domain samples by a gain scalar! This created a 3.2 kHz time-chopper modulation buzz, severe distortion, and only 2.4 dB of noise reduction.
- **Fix**: Integrate `realfft` (R2C forward real FFT, 481 complex bins) with 32 ERB subband grouping, minimum statistics noise floor tracking, Wiener gain calculation, C2R inverse real FFT, and 50% square-root Hann overlap-add synthesis.

---

### Cause 3: Resampling and Channel Conversion
- **Status**: **OK**
- **Verification**: `AudioResampler` wraps `rubato::FftFixedIn` with high-quality sinc interpolation. Verified via test `test_stress_sample_rate_and_channel_sweep` covering 16 kHz Bluetooth HFP, 44.1 kHz, 48 kHz, and 96 kHz. Channel downmixing uses energy-preserving average $(L+R)/2$ without phase cancellation or volume attenuation.

---

### Cause 4: Engine State & Model Fallback
- **Status**: **OK**
- **Verification**: `AudioPipeline` properly manages persistent engine states across frames. Hidden states in RNNoise and the STFT overlap-add history buffer persist across consecutive frames without resetting. Wet/dry mixing slider accurately blends between dry history and cleaned wet buffers.

---

### Cause 5: Gain & Post-Processing (VAD Silence Gating)
- **Status**: **FOUND BUG (MINOR / TUNING NEEDED)**
- **Hypothesis**: The silence detector VAD in `pipeline.rs` has a hard threshold of -52 dBFS and clears `wet_buffer` to 0.0 + comfort noise immediately when inactive, cutting off low-energy speech onsets and consonant tails.
- **Fix**: Couple the VAD decision with the engine's internal neural/spectral VAD estimate, increase hangover time to 300 ms, and maintain circular pre-roll so speech syllables are never muted.

---

### Cause 6: Threading & Buffer Underrun / Overflow
- **Status**: **OK**
- **Verification**: Audio streams use lock-free SPSC ring buffers (`ringbuf`). Verified through stress tests: `test_stress_live_monitor_tap_concurrency` (zero race conditions), `test_pipeline_no_reallocations_over_1000_frames` (0 allocations), and MMCSS `Pro Audio` thread priority setting.

---

### Cause 7: Device Routing and Virtual Cable
- **Status**: **OK**
- **Verification**: `DeviceManager` explicitly writes to `CABLE Input` (the playback render endpoint) so that third-party applications (Discord, Zoom, Teams) receive cleaned audio on `CABLE Output` (the capture recording endpoint). UI banner guides the user with a download link and driver verification.

---

### Cause 8: Windows Audio Settings & Privacy Permissions
- **Status**: **NEW FEATURE IMPLEMENTED**
- **Resolution**: Implemented Windows Audio Diagnostic checks:
  1. Microphone privacy permissions in Windows Settings (`HKCU\...\ConsentStore\microphone`).
  2. Virtual cable installation status.
  3. Sample rate consistency (48 kHz).
  4. Exposed via Tauri command `check_windows_audio_diagnostics` and CLI.

---

### Cause 9: Third-Party Meeting App Conflicts
- **Status**: **DOCUMENTED & UI WARNING ADDED**
- **Resolution**: Discord, Zoom, and Teams have their own built-in noise suppression (Krisp/RNNoise). When both QuietMic and meeting apps run AI denoisers simultaneously, double-filtering causes muffled/underwater audio. Added clear UI guidance: *"Disable noise cancellation in your meeting app (Discord/Zoom/Teams) for best voice clarity."*

---

## 4. Next Steps
1. Apply sample scaling fix in `LightEngine`.
2. Implement 960-point RealFFT spectral Wiener filter in `StandardEngine`.
3. Tune VAD silence hangover in `pipeline.rs`.
4. Re-run 8-stage diagnostic sweep and verify $\ge 15$ dB noise attenuation.
5. Generate A/B WAV files in `ab_samples/`.
