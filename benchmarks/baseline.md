# QuietMic Baseline Performance Benchmark

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Executive Performance Summary

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **40.9 µs** | 86.6 µs | 184.0 µs | **0.0041** | **0** | 33.66 MB |
| **Light (RNNoise)** | Yes | **41.1 µs** | 72.1 µs | 134.0 µs | **0.0042** | **0** | 33.66 MB |
| **Standard (DeepFilterNet3)** | No | **7.4 µs** | 14.0 µs | 83.0 µs | **0.0008** | **0** | 33.66 MB |
| **Standard (DeepFilterNet3)** | Yes | **7.5 µs** | 13.6 µs | 96.0 µs | **0.0009** | **0** | 33.66 MB |
| **Voice Isolation** | No | **45.6 µs** | 66.5 µs | 113.0 µs | **0.0046** | **0** | 33.66 MB |
| **Voice Isolation** | Yes | **46.9 µs** | 79.0 µs | 117.0 µs | **0.0048** | **0** | 33.66 MB |

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
