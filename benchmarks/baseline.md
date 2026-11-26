# 180Hz NC Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **54.6 µs** | 75.4 µs | 220.0 µs | **0.0055** | **0** | 34.56 MB |
| **Light (RNNoise)** | Yes | **57.3 µs** | 78.7 µs | 290.0 µs | **0.0058** | **0** | 34.56 MB |
| **Standard (DeepFilterNet3)** | No | **27.9 µs** | 39.2 µs | 114.0 µs | **0.0028** | **0** | 34.56 MB |
| **Standard (DeepFilterNet3)** | Yes | **30.3 µs** | 48.8 µs | 205.0 µs | **0.0031** | **0** | 34.56 MB |
| **Voice Isolation** | No | **60.8 µs** | 97.5 µs | 275.0 µs | **0.0061** | **0** | 34.56 MB |
| **Voice Isolation** | Yes | **60.7 µs** | 85.8 µs | 200.0 µs | **0.0061** | **0** | 34.56 MB |

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
