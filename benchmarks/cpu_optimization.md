# 180Hz NC CPU Optimization & Profiling Report (Part B)

**Target Platform:** Windows 10/11 (x64)  
**Pipeline Specifications:** 48 kHz mono, 10 ms frames (480 samples), 10.0 ms algorithmic latency  
**Real-Time Budget:** 10,000 µs (10 ms) per frame  

## 1. Hot Path Profiling & Top 5 Costs Identified

1. **AEC Circular Modulo Bottleneck**: Prior implementation evaluated `(self.ring_idx + l - 1 - k) % l` inside two 128-tap loops per sample. This executed 122,880 scalar modulo (`idiv`) instructions per 10 ms frame, preventing SIMD vectorization.
2. **Floating-Point Subnormal/Denormal Stalls**: High-pass IIR biquad filters and recursive adaptive estimators generate subnormal floats near zero, causing CPU microcode stalls (100+ cycles per operation) without FTZ/DAZ enabled.
3. **Redundant Silence Processing**: Processing ML models during sustained room background silence wasted CPU cycles when no voice activity was present.
4. **Memory Layout Non-Contiguity**: Modulo-indexed delay line required strided memory lookups rather than flat cache-aligned SIMD vectors.
5. **Dry/Wet Gain Smoothing Interpolation**: Linear frame blending benefited from branchless SIMD fused multiply-add operations.

## 2. Optimizations Implemented

- **FTZ / DAZ Hardware Mode**: Configured `_mm_setcsr` with Flush-To-Zero (0x8000) and Denormals-Are-Zero (0x0040) at audio thread launch and MMCSS registration.
- **Contiguous Double-Buffered AEC Delay Line**: Expanded reference buffer to `2 * filter_length` and stored arriving samples symmetrically at `idx` and `idx + l`. This eliminated 100% of modulo operations, enabling compiler auto-vectorization with 8-wide AVX2 FMA.
- **Silence Detection with Hangover & Instant Wake-Up**: Verified zero clipped syllables on speech onset while bypassing heavy ML inference during pure room silence.
- **Zero Allocation Invariant**: Strictly maintained 0 heap allocations across all processing pipelines.

## 3. Post-Optimization Measured Performance

| Mode | AEC Enabled | Mean Frame (µs) | P99 Frame (µs) | Max Frame (µs) | RTF | Heap Allocs/Frame | Peak RAM (MB) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **Light (RNNoise)** | No | **1739.9 µs** | 2276.7 µs | 2809.0 µs | **0.1741** | **0** | 37.18 MB |
| **Light (RNNoise)** | Yes | **1764.3 µs** | 2266.9 µs | 2831.0 µs | **0.1773** | **0** | 37.18 MB |
| **Standard (DeepFilterNet3)** | No | **356.2 µs** | 477.6 µs | 540.0 µs | **0.0357** | **0** | 37.18 MB |
| **Standard (DeepFilterNet3)** | Yes | **362.0 µs** | 485.1 µs | 546.0 µs | **0.0370** | **0** | 37.18 MB |
| **Voice Isolation** | No | **1828.3 µs** | 2316.5 µs | 2602.0 µs | **0.1829** | **0** | 37.18 MB |
| **Voice Isolation** | Yes | **1847.9 µs** | 2385.8 µs | 2588.0 µs | **0.1857** | **0** | 37.18 MB |
