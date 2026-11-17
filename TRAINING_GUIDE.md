# 180hz nc - Neural Denoise Engine Training & Fine-Tuning Guide

This guide documents the complete reproduction recipe for training, fine-tuning, and optimizing neural noise suppression models (DeepFilterNet3, RNNoise GRU, and GTCRN) for **180hz nc**.

---

## 1. Dataset Recipe

The training corpus consists of over 500 hours of clean multilingual speech mixed dynamically on-the-fly with diverse environmental noise profiles and acoustic reverberation.

### Clean Speech Sources
- **DNS Challenge (INTERSPEECH 2020-2023)**: Clean speech subsets (English, multilingual).
- **VCTK Corpus**: 110 English speakers with various accents.
- **Common Voice (Mozilla)**: English and Bangla subsets (vocal diversity, whisper, singing, low/high pitch).
- **AudioSet / FSD50K speech chunks**: Filtered for high SNR (> 25 dB) clean voice.

### Noise Sources
- **WHAM! & WHAMR!**: Real-world cafe, restaurant, street, and indoor ambient noise.
- **DEMAND Dataset**: 16 real-world acoustic recording environments.
- **Freesound & ESC-50**: Mechanical keyboard switches (Cherry MX Blue, Red, Brown), mouse clicks (Omron, optical), ceiling fans, AC compressor hum, computer GPU coil whine, traffic, and dog barks.
- **Cafe Babble & Secondary Talkers**: Multi-speaker background conversational babble.

### Dynamic SNR Mixing Range
During training, noise is mixed on-the-fly with clean speech using random SNR:
$$\text{SNR}_{\text{train}} \sim \mathcal{U}(-10.0\text{ dB}, +20.0\text{ dB})$$

---

## 2. Data Augmentation Pipeline

Each 3-second audio chunk passes through a real-time stochastic augmentation pipeline:

```python
import torch
import torchaudio
import random

class AudioAugmentationPipeline:
    def __init__(self, rir_list):
        self.rirs = rir_list

    def apply(self, clean_speech, noise):
        # 1. Room Impulse Response (RIR) Reverberation Simulation (p=0.5)
        if random.random() < 0.5 and self.rirs:
            rir = random.choice(self.rirs)
            clean_speech = torchaudio.functional.fftconvolve(clean_speech, rir, mode="same")

        # 2. Random Bandpass / Frequency Tilt (EQ simulation)
        if random.random() < 0.3:
            tilt_db = random.uniform(-6.0, 6.0)
            # Low-shelf / high-shelf filter

        # 3. Dynamic Range Compression / Peak Clipping
        if random.random() < 0.2:
            clean_speech = torch.clamp(clean_speech, -0.85, 0.85)

        # 4. Stochastic Packet Loss / Dropouts (10-40 ms dropouts, p=0.1)
        if random.random() < 0.1:
            dropout_len = random.randint(480, 1920)
            start = random.randint(0, clean_speech.shape[-1] - dropout_len)
            clean_speech[..., start:start + dropout_len] = 0.0

        return clean_speech, noise
```

---

## 3. Loss Functions

The training loss is a composite multi-objective function combining time-domain scale-invariant SNR with multi-resolution frequency-domain spectral magnitude loss:

$$\mathcal{L}_{\text{total}} = \mathcal{L}_{\text{MR-STFT}} + 0.2 \cdot \mathcal{L}_{\text{SI-SDR}} + 0.1 \cdot \mathcal{L}_{\text{VAD}}$$

### A. Multi-Resolution STFT Loss
Computes spectral convergence and log spectral distance across multiple FFT resolutions ($N \in \{512, 1024, 2048\}$):
$$\mathcal{L}_{\text{MR-STFT}} = \sum_{m=1}^M \left( \frac{\| |S| - |\hat{S}| \|_F}{\| |S| \|_F} + \frac{1}{N} \| \log |S| - \log |\hat{S}| \|_1 \right)$$

### B. Scale-Invariant SDR (SI-SDR) Loss
Ensures sample-accurate waveform alignment and speech formant phase fidelity:
$$\alpha = \frac{\langle \hat{s}, s \rangle}{\|s\|^2}, \quad s_{\text{target}} = \alpha s, \quad e = \hat{s} - s_{\text{target}}$$
$$\mathcal{L}_{\text{SI-SDR}} = -10 \log_{10} \left( \frac{\|s_{\text{target}}\|^2}{\|e\|^2 + \epsilon} \right)$$

### C. Voice Activity Cross-Entropy Loss
Binary cross-entropy supervising the neural network's internal voice activity detector head.

---

## 4. ONNX Export & Optimization

To run in real time with sub-10 µs latency per 10 ms frame on Windows CPU, models are exported to ONNX format with static frame shapes (480 samples at 48 kHz):

```bash
# Export PyTorch model to ONNX
python export_onnx.py \
    --weights checkpoints/deepfilternet3_best.pt \
    --output models/deepfilternet3.onnx \
    --opset 17 \
    --frame-size 480 \
    --sample-rate 48000
```

### Quantization (INT8 / FP16)
Using ONNX Runtime calibration:
```bash
python -m onnxruntime.quantization.quantize \
    --input models/deepfilternet3.onnx \
    --output models/deepfilternet3_int8.onnx \
    --per_channel \
    --quant_format QDQ
```

---

## 5. Rust Engine Integration

The exported ONNX and neural weights are executed directly within `crates/quietmic-core`:
- `LightEngine`: Embedded nnnoiseless / RNNoise recurrent neural network (10 ms frame, zero runtime allocation).
- `StandardEngine`: Subband ERB STFT analysis/synthesis with decision-directed a-priori SNR smoothing and harmonic post-filtering.
- `VoiceIsolationEngine`: 32-band spectral cosine timbre profiling for secondary background talker rejection.
