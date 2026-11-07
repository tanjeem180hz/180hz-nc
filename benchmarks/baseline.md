# QuietMic Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **66.5 µs** | 144.1 µs | 1374.0 µs | **0.0067** | **0** | 32.18 MB |
| **Light (RNNoise)** | Yes | **24.8 µs** | 98.8 µs | 268.0 µs | **0.0026** | **0** | 32.18 MB |
| **Standard (DeepFilterNet3)** | No | **8.7 µs** | 19.9 µs | 146.0 µs | **0.0009** | **0** | 32.18 MB |
| **Standard (DeepFilterNet3)** | Yes | **17.0 µs** | 48.0 µs | 243.0 µs | **0.0018** | **0** | 32.18 MB |
| **Voice Isolation** | No | **68.0 µs** | 127.5 µs | 538.0 µs | **0.0069** | **0** | 32.18 MB |
| **Voice Isolation** | Yes | **27.3 µs** | 97.2 µs | 211.0 µs | **0.0029** | **0** | 32.18 MB |

## 2. Objective Speech Quality & Noise Suppression Matrix

| Scenario | Mode | AEC | Input SNR | Output SNR | ΔSNR | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | -2.9 dB | **2.1 dB** | 1.51 | 2.10 | **1.77** |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | -2.6 dB | **2.4 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | -2.8 dB | **2.2 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | -3.4 dB | **1.6 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | No | -0.0 dB | -2.9 dB | **-2.9 dB** | 1.51 | 2.10 | **1.77** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | Yes | -0.0 dB | -2.6 dB | **-2.6 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | No | -0.0 dB | -2.7 dB | **-2.7 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | Yes | -0.0 dB | -3.4 dB | **-3.4 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | No | 5.0 dB | -3.0 dB | **-8.0 dB** | 1.49 | 2.10 | **1.77** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | Yes | 5.0 dB | -2.6 dB | **-7.6 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | No | 5.0 dB | -2.7 dB | **-7.7 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | Yes | 5.0 dB | -3.4 dB | **-8.4 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | No | 10.0 dB | -2.9 dB | **-12.9 dB** | 1.48 | 2.10 | **1.76** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | Yes | 10.0 dB | -2.6 dB | **-12.6 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | No | 10.0 dB | -2.7 dB | **-12.7 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | Yes | 10.0 dB | -3.4 dB | **-13.4 dB** | 1.20 | 2.10 | **1.61** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | -1.0 dB | **4.0 dB** | 1.50 | 2.10 | **1.77** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | -2.7 dB | **2.3 dB** | 1.20 | 2.10 | **1.61** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | -4.8 dB | **0.2 dB** | 1.20 | 2.10 | **1.61** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | -3.3 dB | **1.7 dB** | 1.20 | 2.10 | **1.61** |
