# 180Hz NC Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **1739.9 µs** | 2276.7 µs | 2809.0 µs | **0.1741** | **0** | 37.18 MB |
| **Light (RNNoise)** | Yes | **1764.3 µs** | 2266.9 µs | 2831.0 µs | **0.1773** | **0** | 37.18 MB |
| **Standard (DeepFilterNet3)** | No | **356.2 µs** | 477.6 µs | 540.0 µs | **0.0357** | **0** | 37.18 MB |
| **Standard (DeepFilterNet3)** | Yes | **362.0 µs** | 485.1 µs | 546.0 µs | **0.0370** | **0** | 37.18 MB |
| **Voice Isolation** | No | **1828.3 µs** | 2316.5 µs | 2602.0 µs | **0.1829** | **0** | 37.18 MB |
| **Voice Isolation** | Yes | **1847.9 µs** | 2385.8 µs | 2588.0 µs | **0.1857** | **0** | 37.18 MB |

## 2. Objective Speech Quality & Noise Suppression Matrix

| Scenario | Mode | AEC | Input SNR | Output SNR | ΔSNR | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 3.7 dB | **8.7 dB** | 3.29 | 2.65 | **3.00** |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 3.7 dB | **8.7 dB** | 3.29 | 2.65 | **3.00** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 4.4 dB | **9.4 dB** | 3.34 | 2.73 | **3.07** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 4.4 dB | **9.4 dB** | 3.34 | 2.73 | **3.07** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | No | -0.0 dB | 4.4 dB | **4.4 dB** | 3.34 | 2.73 | **3.07** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | Yes | -0.0 dB | 4.4 dB | **4.4 dB** | 3.34 | 2.73 | **3.07** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | No | -0.0 dB | 4.7 dB | **4.7 dB** | 3.37 | 2.76 | **3.09** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | Yes | -0.0 dB | 4.7 dB | **4.7 dB** | 3.37 | 2.76 | **3.09** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | No | 5.0 dB | 4.8 dB | **-0.2 dB** | 3.38 | 2.78 | **3.11** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | Yes | 5.0 dB | 4.8 dB | **-0.2 dB** | 3.38 | 2.78 | **3.11** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | No | 5.0 dB | 5.1 dB | **0.1 dB** | 3.39 | 2.80 | **3.12** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | Yes | 5.0 dB | 5.1 dB | **0.1 dB** | 3.39 | 2.80 | **3.12** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | No | 10.0 dB | 5.1 dB | **-4.9 dB** | 3.39 | 2.81 | **3.13** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | Yes | 10.0 dB | 5.1 dB | **-4.9 dB** | 3.39 | 2.81 | **3.13** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | No | 10.0 dB | 5.5 dB | **-4.5 dB** | 3.41 | 2.84 | **3.15** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | Yes | 10.0 dB | 5.5 dB | **-4.5 dB** | 3.41 | 2.84 | **3.15** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 1.7 dB | **6.7 dB** | 3.06 | 2.27 | **2.70** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 1.7 dB | **6.7 dB** | 3.06 | 2.27 | **2.70** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 2.0 dB | **7.0 dB** | 3.17 | 2.45 | **2.84** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 2.0 dB | **7.0 dB** | 3.17 | 2.45 | **2.84** |
