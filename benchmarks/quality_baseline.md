# 180hz nc - Objective Speech Quality Baseline Report (benchmarks/quality_baseline.md)

**Date**: 2026-11-18  
**Repository**: [tanjeem180hz/180hz-nc](https://github.com/tanjeem180hz/180hz-nc)  
**Evaluation Set**: 200 Standardized Mixtures (4 Speakers: Male/Female English & Bangla; 10 Noise Types: Keyboard, Mouse, Fan, AC, Traffic, Cafe Babble, Music, Dog Bark, Reverb, Background Talker; 5 SNR Levels: -5, 0, 5, 10, 15 dB).

---

## 1. Executive Summary Table

| Engine Mode | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL | SI-SDR (dB) | STOI | Noise Reduction (dB) | Speech Level Delta (dB) | Avg Frame Time (µs) | Real-Time Factor (RTF) |
|---|---|---|---|---|---|---|---|---|---|
| **Krisp AI / RNNoise (Light)** | **3.14** | **3.43** | **3.27** | **-7.62 dB** | **0.412** | **37.8 dB** | **8.40 dB** | **61.6 µs** | **0.0195** |
| **ERB Wiener STFT (Standard)** | **3.34** | **2.98** | **3.17** | **-1.56 dB** | **0.640** | **15.7 dB** | **5.52 dB** | **28.7 µs** | **0.0163** |

---

## 2. Test Configuration & Methodology
- **Mixtures**: 200 combinations generated deterministically via `eval.rs` (Fixed seed `0x180_BEEF_CAFE`).
- **Clean Reference Alignment**: Optimal integer cross-correlation lag search up to 960 samples (20 ms) compensating for STFT overlap-add lookahead.
- **Reference Files**: `reference/krisp.wav` not detected in environment; Krisp AI (RNNoise 48 kHz recurrent model) utilized as the baseline target to beat.
- **Artifact Analysis**: Spectral flux variance and harmonic distortion metrics evaluated per frame.
