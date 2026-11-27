# 180Hz NC Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **64.6 µs** | 174.9 µs | 332.0 µs | **0.0065** | **0** | 33.98 MB |
| **Light (RNNoise)** | Yes | **65.7 µs** | 160.2 µs | 337.0 µs | **0.0067** | **0** | 33.98 MB |
| **Standard (DeepFilterNet3)** | No | **33.0 µs** | 101.2 µs | 284.0 µs | **0.0034** | **0** | 33.98 MB |
| **Standard (DeepFilterNet3)** | Yes | **35.8 µs** | 115.9 µs | 289.0 µs | **0.0037** | **0** | 33.98 MB |
| **Voice Isolation** | No | **66.8 µs** | 169.0 µs | 311.0 µs | **0.0068** | **0** | 33.98 MB |
| **Voice Isolation** | Yes | **69.8 µs** | 137.2 µs | 268.0 µs | **0.0071** | **0** | 33.98 MB |

## 2. Objective Speech Quality & Noise Suppression Matrix

| Scenario | Mode | AEC | Input SNR | Output SNR | ΔSNR | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 3.4 dB | **8.4 dB** | 3.25 | 2.58 | **2.95** |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 3.4 dB | **8.4 dB** | 3.25 | 2.58 | **2.95** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 5.1 dB | **10.1 dB** | 3.38 | 2.78 | **3.11** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 5.1 dB | **10.1 dB** | 3.38 | 2.78 | **3.11** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | No | -0.0 dB | 3.6 dB | **3.6 dB** | 3.26 | 2.60 | **2.96** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | Yes | -0.0 dB | 3.6 dB | **3.6 dB** | 3.26 | 2.60 | **2.96** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | No | -0.0 dB | 5.4 dB | **5.4 dB** | 3.40 | 2.81 | **3.13** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | Yes | -0.0 dB | 5.4 dB | **5.4 dB** | 3.40 | 2.81 | **3.13** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | No | 5.0 dB | 3.9 dB | **-1.1 dB** | 3.28 | 2.64 | **2.99** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | Yes | 5.0 dB | 3.9 dB | **-1.1 dB** | 3.28 | 2.64 | **2.99** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | No | 5.0 dB | 5.7 dB | **0.7 dB** | 3.42 | 2.85 | **3.16** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | Yes | 5.0 dB | 5.7 dB | **0.7 dB** | 3.42 | 2.85 | **3.16** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | No | 10.0 dB | 4.0 dB | **-6.0 dB** | 3.29 | 2.65 | **3.00** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | Yes | 10.0 dB | 4.0 dB | **-6.0 dB** | 3.29 | 2.65 | **3.00** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | No | 10.0 dB | 6.0 dB | **-4.0 dB** | 3.44 | 2.88 | **3.19** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | Yes | 10.0 dB | 6.0 dB | **-4.0 dB** | 3.44 | 2.88 | **3.19** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 0.7 dB | **5.7 dB** | 2.81 | 2.10 | **2.49** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 0.7 dB | **5.7 dB** | 2.81 | 2.10 | **2.49** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 4.3 dB | **9.3 dB** | 3.31 | 2.68 | **3.03** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 4.3 dB | **9.3 dB** | 3.31 | 2.68 | **3.03** |
