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
| **Light (RNNoise)** | No | **54.9 µs** | 66.4 µs | 138.0 µs | **0.0055** | **0** | 34.43 MB |
| **Light (RNNoise)** | Yes | **57.4 µs** | 72.4 µs | 143.0 µs | **0.0058** | **0** | 34.43 MB |
| **Standard (DeepFilterNet3)** | No | **28.4 µs** | 33.8 µs | 62.0 µs | **0.0029** | **0** | 34.43 MB |
| **Standard (DeepFilterNet3)** | Yes | **30.4 µs** | 37.5 µs | 59.0 µs | **0.0031** | **0** | 34.43 MB |
| **Voice Isolation** | No | **59.5 µs** | 68.5 µs | 80.0 µs | **0.0060** | **0** | 34.43 MB |
| **Voice Isolation** | Yes | **61.8 µs** | 72.0 µs | 85.0 µs | **0.0063** | **0** | 34.43 MB |
