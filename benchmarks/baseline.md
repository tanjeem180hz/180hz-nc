# 180Hz NC Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **63.5 µs** | 96.7 µs | 256.0 µs | **0.0064** | **0** | 34.16 MB |
| **Light (RNNoise)** | Yes | **64.2 µs** | 110.5 µs | 213.0 µs | **0.0065** | **1** | 34.16 MB |
| **Standard (DeepFilterNet3)** | No | **33.1 µs** | 59.8 µs | 192.0 µs | **0.0034** | **0** | 34.16 MB |
| **Standard (DeepFilterNet3)** | Yes | **37.5 µs** | 75.5 µs | 271.0 µs | **0.0038** | **1** | 34.16 MB |
| **Voice Isolation** | No | **71.1 µs** | 117.5 µs | 256.0 µs | **0.0072** | **1** | 34.16 MB |
| **Voice Isolation** | Yes | **70.4 µs** | 114.8 µs | 218.0 µs | **0.0071** | **2** | 34.16 MB |

## 2. Objective Speech Quality & Noise Suppression Matrix

| Scenario | Mode | AEC | Input SNR | Output SNR | ΔSNR | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 4.4 dB | **9.4 dB** | 3.34 | 2.73 | **3.07** |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 4.4 dB | **9.4 dB** | 3.34 | 2.73 | **3.07** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 5.1 dB | **10.1 dB** | 3.40 | 2.82 | **3.14** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 5.1 dB | **10.1 dB** | 3.40 | 2.82 | **3.14** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | No | -0.0 dB | 4.7 dB | **4.7 dB** | 3.37 | 2.77 | **3.10** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | Yes | -0.0 dB | 4.7 dB | **4.7 dB** | 3.37 | 2.77 | **3.10** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | No | -0.0 dB | 5.6 dB | **5.6 dB** | 3.42 | 2.86 | **3.17** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | Yes | -0.0 dB | 5.6 dB | **5.6 dB** | 3.42 | 2.86 | **3.17** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | No | 5.0 dB | 5.0 dB | **0.0 dB** | 3.39 | 2.80 | **3.12** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | Yes | 5.0 dB | 5.0 dB | **0.0 dB** | 3.39 | 2.80 | **3.12** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | No | 5.0 dB | 6.0 dB | **1.0 dB** | 3.45 | 2.90 | **3.20** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | Yes | 5.0 dB | 6.0 dB | **1.0 dB** | 3.45 | 2.90 | **3.20** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | No | 10.0 dB | 5.3 dB | **-4.7 dB** | 3.40 | 2.82 | **3.14** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | Yes | 10.0 dB | 5.3 dB | **-4.7 dB** | 3.40 | 2.82 | **3.14** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | No | 10.0 dB | 6.5 dB | **-3.5 dB** | 3.48 | 2.94 | **3.24** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | Yes | 10.0 dB | 6.5 dB | **-3.5 dB** | 3.48 | 2.94 | **3.24** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 1.0 dB | **6.0 dB** | 2.91 | 2.10 | **2.54** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 1.0 dB | **6.0 dB** | 2.91 | 2.10 | **2.54** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 2.0 dB | **7.0 dB** | 3.20 | 2.50 | **2.88** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 2.0 dB | **7.0 dB** | 3.20 | 2.50 | **2.88** |
