# QuietMic Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **75.9 µs** | 368.3 µs | 1710.0 µs | **0.0077** | **0** | 32.20 MB |
| **Light (RNNoise)** | Yes | **81.4 µs** | 436.9 µs | 21211.0 µs | **0.0083** | **0** | 32.20 MB |
| **Standard (DeepFilterNet3)** | No | **13.0 µs** | 68.1 µs | 597.0 µs | **0.0014** | **0** | 32.20 MB |
| **Standard (DeepFilterNet3)** | Yes | **12.9 µs** | 73.2 µs | 711.0 µs | **0.0014** | **0** | 32.20 MB |
| **Voice Isolation** | No | **80.3 µs** | 411.5 µs | 1582.0 µs | **0.0081** | **0** | 32.20 MB |
| **Voice Isolation** | Yes | **79.5 µs** | 414.8 µs | 1296.0 µs | **0.0081** | **0** | 32.20 MB |

## 2. Objective Speech Quality & Noise Suppression Matrix

| Scenario | Mode | AEC | Input SNR | Output SNR | ΔSNR | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 3.7 dB | **8.7 dB** | 3.29 | 2.65 | **3.00** |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 3.7 dB | **8.7 dB** | 3.29 | 2.65 | **3.00** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 4.4 dB | **9.4 dB** | 3.35 | 2.73 | **3.07** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 4.4 dB | **9.4 dB** | 3.35 | 2.73 | **3.07** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | No | -0.0 dB | 4.4 dB | **4.4 dB** | 3.34 | 2.73 | **3.06** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | Yes | -0.0 dB | 4.4 dB | **4.4 dB** | 3.34 | 2.73 | **3.06** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | No | -0.0 dB | 4.7 dB | **4.7 dB** | 3.37 | 2.77 | **3.10** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | Yes | -0.0 dB | 4.7 dB | **4.7 dB** | 3.37 | 2.77 | **3.10** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | No | 5.0 dB | 4.8 dB | **-0.2 dB** | 3.38 | 2.78 | **3.11** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | Yes | 5.0 dB | 4.8 dB | **-0.2 dB** | 3.38 | 2.78 | **3.11** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | No | 5.0 dB | 5.1 dB | **0.1 dB** | 3.39 | 2.80 | **3.13** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | Yes | 5.0 dB | 5.1 dB | **0.1 dB** | 3.39 | 2.80 | **3.13** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | No | 10.0 dB | 5.1 dB | **-4.9 dB** | 3.39 | 2.81 | **3.13** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | Yes | 10.0 dB | 5.1 dB | **-4.9 dB** | 3.39 | 2.81 | **3.13** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | No | 10.0 dB | 5.5 dB | **-4.5 dB** | 3.41 | 2.84 | **3.15** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | Yes | 10.0 dB | 5.5 dB | **-4.5 dB** | 3.41 | 2.84 | **3.15** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 1.8 dB | **6.8 dB** | 3.07 | 2.29 | **2.72** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 1.8 dB | **6.8 dB** | 3.07 | 2.29 | **2.72** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 2.0 dB | **7.0 dB** | 3.17 | 2.44 | **2.84** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 2.0 dB | **7.0 dB** | 3.17 | 2.44 | **2.84** |
