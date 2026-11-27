# 180Hz NC Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **54.8 µs** | 74.6 µs | 302.0 µs | **0.0055** | **0** | 33.51 MB |
| **Light (RNNoise)** | Yes | **56.8 µs** | 74.4 µs | 144.0 µs | **0.0058** | **0** | 33.51 MB |
| **Standard (DeepFilterNet3)** | No | **27.7 µs** | 34.9 µs | 68.0 µs | **0.0028** | **0** | 33.51 MB |
| **Standard (DeepFilterNet3)** | Yes | **30.0 µs** | 37.2 µs | 57.0 µs | **0.0031** | **0** | 33.51 MB |
| **Voice Isolation** | No | **59.0 µs** | 81.2 µs | 161.0 µs | **0.0059** | **0** | 33.51 MB |
| **Voice Isolation** | Yes | **62.1 µs** | 84.5 µs | 132.0 µs | **0.0063** | **0** | 33.51 MB |

## 2. Objective Speech Quality & Noise Suppression Matrix

| Scenario | Mode | AEC | Input SNR | Output SNR | ΔSNR | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 3.6 dB | **8.6 dB** | 3.28 | 2.63 | **2.99** |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 3.6 dB | **8.6 dB** | 3.28 | 2.63 | **2.99** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 4.9 dB | **9.9 dB** | 3.39 | 2.80 | **3.12** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 4.9 dB | **9.9 dB** | 3.39 | 2.80 | **3.12** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | No | -0.0 dB | 4.4 dB | **4.4 dB** | 3.34 | 2.73 | **3.07** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | Yes | -0.0 dB | 4.4 dB | **4.4 dB** | 3.34 | 2.73 | **3.07** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | No | -0.0 dB | 5.4 dB | **5.4 dB** | 3.41 | 2.84 | **3.15** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | Yes | -0.0 dB | 5.4 dB | **5.4 dB** | 3.41 | 2.84 | **3.15** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | No | 5.0 dB | 4.7 dB | **-0.3 dB** | 3.37 | 2.77 | **3.10** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | Yes | 5.0 dB | 4.7 dB | **-0.3 dB** | 3.37 | 2.77 | **3.10** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | No | 5.0 dB | 5.8 dB | **0.8 dB** | 3.44 | 2.88 | **3.19** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | Yes | 5.0 dB | 5.8 dB | **0.8 dB** | 3.44 | 2.88 | **3.19** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | No | 10.0 dB | 4.8 dB | **-5.2 dB** | 3.36 | 2.76 | **3.09** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | Yes | 10.0 dB | 4.8 dB | **-5.2 dB** | 3.36 | 2.76 | **3.09** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | No | 10.0 dB | 6.4 dB | **-3.6 dB** | 3.47 | 2.93 | **3.23** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | Yes | 10.0 dB | 6.4 dB | **-3.6 dB** | 3.47 | 2.93 | **3.23** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 1.0 dB | **6.0 dB** | 2.90 | 2.10 | **2.54** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 1.0 dB | **6.0 dB** | 2.90 | 2.10 | **2.54** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 4.0 dB | **9.0 dB** | 3.30 | 2.66 | **3.01** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 4.0 dB | **9.0 dB** | 3.30 | 2.66 | **3.01** |
