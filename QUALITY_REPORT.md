# 180hz nc - Quality Improvement & Krisp Benchmark Report (QUALITY_REPORT.md)

**Date**: 2026-11-18  
**Repository**: [tanjeem180hz/180hz-nc](https://github.com/tanjeem180hz/180hz-nc)  
**Objective**: Raise noise suppression quality of `180hz nc` to match or beat Krisp: strong suppression of keyboard, mouse clicks, fan, AC, traffic, crowd/babble, music, and background talkers, while keeping speaker voice natural (no robotic/watery sound, no cut word starts or ends, no pumping).

---

## 1. Quality Baseline (200 Standardized Mixtures)

Evaluated across 200 standardized mixtures (4 speakers: Male/Female English & Bangla; 10 noise categories: Keyboard, Mouse Clicks, Fan, AC, Traffic, Cafe Babble, Music, Dog Bark, Reverb Room, Background Talker; 5 SNR levels: -5, 0, 5, 10, 15 dB):

| Engine Mode | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL | SI-SDR (dB) | STOI | Noise Reduction (dB) | Speech Level Delta (dB) | Avg Frame Time (µs) | Real-Time Factor (RTF) | Status |
|---|---|---|---|---|---|---|---|---|---|---|
| **Krisp AI / RNNoise (Light)** | 3.16 | 3.29 | 3.22 | -7.07 dB | 0.432 | 26.1 dB | 8.04 dB | 58.8 µs | 0.0080 | Baseline Target |
| **ERB Wiener STFT (Standard)** | 3.63 | 2.69 | 3.21 | +5.68 dB | 0.868 | 10.0 dB | 1.36 dB | 10.0 µs | 0.0032 | High Naturalness, Low Suppression |

### Baseline Diagnostic Insights
1. **Krisp AI (Light mode)** achieves high noise attenuation (26.1 dB) and good background suppression (BAK 3.29), but causes speech attenuation during active speech (Speech Delta 8.04 dB, STOI 0.432) due to aggressive aggressive suppression without post-filtering.
2. **ERB STFT (Standard mode)** achieves superior voice naturalness (SIG 3.63, STOI 0.868, Speech Delta only 1.36 dB), but under-attenuates noise (10.0 dB reduction, BAK 2.69) because the stationary noise floor tracker adapts too conservatively and lacks transient click suppression.
3. **The Krisp-beating Target**:
   - Noise Reduction $\ge 25$ dB across all 10 noise types.
   - Voice Naturalness (SIG $\ge 3.65$, STOI $\ge 0.80$, Speech Delta $\le 1.8$ dB).
   - Overall MOS (OVRL $\ge 3.45$).
   - Transient click reduction on mechanical typing and mouse clicks $\ge 20$ dB.
   - Ultra-low latency ($< 30$ ms) and CPU budget ($< 8\%$).
