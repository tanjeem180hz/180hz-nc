# 180Hz NC Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **43.8 µs** | 60.4 µs | 185.0 µs | **0.0044** | **0** | 33.73 MB |
| **Light (RNNoise)** | Yes | **79.0 µs** | 108.0 µs | 190.0 µs | **0.0080** | **0** | 33.73 MB |
| **Standard (DeepFilterNet3)** | No | **9.2 µs** | 14.1 µs | 74.0 µs | **0.0010** | **0** | 33.73 MB |
| **Standard (DeepFilterNet3)** | Yes | **45.1 µs** | 61.1 µs | 305.0 µs | **0.0046** | **0** | 33.73 MB |
| **Voice Isolation** | No | **48.9 µs** | 67.0 µs | 115.0 µs | **0.0049** | **1** | 33.73 MB |
| **Voice Isolation** | Yes | **83.0 µs** | 99.8 µs | 117.0 µs | **0.0084** | **1** | 33.73 MB |

## 2. Objective Speech Quality & Noise Suppression Matrix

| Scenario | Mode | AEC | Input SNR | Output SNR | ΔSNR | DNSMOS SIG | DNSMOS BAK | DNSMOS OVRL |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 4.1 dB | **9.1 dB** | 3.34 | 2.72 | **3.06** |
| Keyboard_Clicks_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 4.1 dB | **9.1 dB** | 3.34 | 2.72 | **3.06** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 4.6 dB | **9.6 dB** | 3.36 | 2.75 | **3.08** |
| Keyboard_Clicks_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 4.6 dB | **9.6 dB** | 3.36 | 2.75 | **3.08** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | No | -0.0 dB | 4.5 dB | **4.5 dB** | 3.35 | 2.74 | **3.08** |
| Keyboard_Clicks_+0dB_SNR | Light (RNNoise) | Yes | -0.0 dB | 4.5 dB | **4.5 dB** | 3.35 | 2.74 | **3.08** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | No | -0.0 dB | 5.0 dB | **5.0 dB** | 3.38 | 2.79 | **3.11** |
| Keyboard_Clicks_+0dB_SNR | Standard (DeepFilterNet3) | Yes | -0.0 dB | 5.0 dB | **5.0 dB** | 3.38 | 2.79 | **3.11** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | No | 5.0 dB | 4.7 dB | **-0.3 dB** | 3.37 | 2.77 | **3.10** |
| Keyboard_Clicks_+5dB_SNR | Light (RNNoise) | Yes | 5.0 dB | 4.7 dB | **-0.3 dB** | 3.37 | 2.77 | **3.10** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | No | 5.0 dB | 5.4 dB | **0.4 dB** | 3.40 | 2.82 | **3.14** |
| Keyboard_Clicks_+5dB_SNR | Standard (DeepFilterNet3) | Yes | 5.0 dB | 5.4 dB | **0.4 dB** | 3.40 | 2.82 | **3.14** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | No | 10.0 dB | 5.0 dB | **-5.0 dB** | 3.38 | 2.79 | **3.12** |
| Keyboard_Clicks_+10dB_SNR | Light (RNNoise) | Yes | 10.0 dB | 5.0 dB | **-5.0 dB** | 3.38 | 2.79 | **3.12** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | No | 10.0 dB | 5.7 dB | **-4.2 dB** | 3.43 | 2.86 | **3.17** |
| Keyboard_Clicks_+10dB_SNR | Standard (DeepFilterNet3) | Yes | 10.0 dB | 5.7 dB | **-4.2 dB** | 3.43 | 2.86 | **3.17** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | No | -5.0 dB | 1.6 dB | **6.6 dB** | 3.03 | 2.23 | **2.67** |
| Fan_AC_Hum_-5dB_SNR | Light (RNNoise) | Yes | -5.0 dB | 1.6 dB | **6.6 dB** | 3.03 | 2.23 | **2.67** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | No | -5.0 dB | 3.0 dB | **8.0 dB** | 3.23 | 2.55 | **2.92** |
| Fan_AC_Hum_-5dB_SNR | Standard (DeepFilterNet3) | Yes | -5.0 dB | 3.0 dB | **8.0 dB** | 3.23 | 2.55 | **2.92** |
