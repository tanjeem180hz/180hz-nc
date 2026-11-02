# QuietMic Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **29.6 µs** | 40.8 µs | 168.0 µs | **0.0030** | **0** | 32.62 MB |
| **Light (RNNoise)** | Yes | **172.8 µs** | 193.9 µs | 379.0 µs | **0.0174** | **0** | 32.62 MB |
| **Standard (DeepFilterNet3)** | No | **5.0 µs** | 6.0 µs | 15.0 µs | **0.0006** | **0** | 32.62 MB |
| **Standard (DeepFilterNet3)** | Yes | **151.8 µs** | 164.8 µs | 273.0 µs | **0.0153** | **0** | 32.62 MB |
| **Voice Isolation** | No | **31.3 µs** | 41.2 µs | 44.0 µs | **0.0032** | **0** | 32.62 MB |
| **Voice Isolation** | Yes | **176.6 µs** | 222.2 µs | 302.0 µs | **0.0178** | **0** | 32.62 MB |

## 2. Objective Speech Quality & Noise Suppression Matrix

| Scenario | Mode | AEC | Input SNR | Output SNR | ΔSNR | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | -2.1 dB | **2.9 dB** | 1.42 | 2.10 | **1.73** |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | -1.7 dB | **3.3 dB** | 1.48 | 2.10 | **1.76** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | -2.8 dB | **2.2 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | -2.0 dB | **3.0 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | No | -0.0 dB | -2.0 dB | **-2.0 dB** | 1.43 | 2.10 | **1.73** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | Yes | -0.0 dB | -1.6 dB | **-1.6 dB** | 1.50 | 2.10 | **1.77** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | No | -0.0 dB | -2.7 dB | **-2.7 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | Yes | -0.0 dB | -2.0 dB | **-2.0 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | No | 5.0 dB | -2.0 dB | **-7.0 dB** | 1.45 | 2.10 | **1.74** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | Yes | 5.0 dB | -1.6 dB | **-6.6 dB** | 1.52 | 2.10 | **1.78** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | No | 5.0 dB | -2.7 dB | **-7.7 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | Yes | 5.0 dB | -1.9 dB | **-6.9 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | No | 10.0 dB | -1.9 dB | **-11.9 dB** | 1.47 | 2.10 | **1.75** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | Yes | 10.0 dB | -1.5 dB | **-11.5 dB** | 1.54 | 2.10 | **1.79** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | No | 10.0 dB | -2.7 dB | **-12.7 dB** | 1.20 | 2.10 | **1.61** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | Yes | 10.0 dB | -1.9 dB | **-11.9 dB** | 1.20 | 2.10 | **1.61** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | -1.5 dB | **3.5 dB** | 1.20 | 2.10 | **1.61** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | -1.8 dB | **3.2 dB** | 1.36 | 2.10 | **1.69** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | -4.8 dB | **0.2 dB** | 1.20 | 2.10 | **1.61** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | -2.2 dB | **2.8 dB** | 1.21 | 2.10 | **1.61** |
