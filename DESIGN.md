# 180hz nc UI Design System & Architecture

## 1. Concept & Identity
**180hz nc** features a clean, professional, commercial-grade audio control center built on a unified card-based architecture.

The name refers to **180 Hz**, the acoustic sweet spot of human speech fundamentals and the anchor frequency for speech-noise separation.

> [!NOTE]
> The experimental "180 Hz Signal Lab" canvas oscilloscope design remains fully preserved in the remote git branch `origin/theme-signal-lab`. On `main`, the streamlined, high-efficiency classic card layout is the primary design, enhanced with modern design tokens, Dark & Light mode toggling, and comprehensive diagnostics.

---

## 2. Design Tokens & Visual Hierarchy

### 2.1 Color Palette & Design Tokens
The application implements CSS Custom Properties for immediate theme switching and optimal contrast:

| Token | Dark Theme (Default) | Light Theme (`daylight`) | Purpose |
| :--- | :--- | :--- | :--- |
| `--bg-app` | `#0f172a` (Slate 900) | `#f1f5f9` (Slate 100) | App window canvas |
| `--bg-card` | `#1e293b` (Slate 800) | `#ffffff` (Pure White) | Card surfaces |
| `--bg-input` | `#0b1120` (Slate 950) | `#f8fafc` (Slate 50) | Recessed inputs, dropdowns, meters |
| `--border-card` | `#334155` (Slate 700) | `#e2e8f0` (Slate 200) | Subtle card borders |
| `--accent-emerald` | `#10b981` (Emerald 500) | `#059669` (Emerald 600) | Active status, clean voice, healthy |
| `--accent-blue` | `#3b82f6` (Blue 500) | `#2563eb` (Blue 600) | Primary actions, sliders, active segment |
| `--accent-amber` | `#f59e0b` (Amber 500) | `#d97706` (Amber 600) | Bypassed state, loop warnings |
| `--accent-red` | `#ef4444` (Red 500) | `#dc2626` (Red 600) | Overload clipping alert, error states |
| `--text-primary` | `#f8fafc` (Slate 50) | `#0f172a` (Slate 900) | High-contrast readable typography |
| `--text-secondary` | `#94a3b8` (Slate 400) | `#475569` (Slate 600) | Labels, descriptors, micro-copy |

### 2.2 Typography & Spacing
- **Font Stack**: Clean system font stack (`-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `Roboto`, `sans-serif`) for crisp rendering at any DPI.
- **Monospace Stack**: High-legibility mono numbers for dB scales, latency readouts, and frame counters.
- **Grid & Spacing**: Strict 4/8 px grid scale (4px, 8px, 12px, 16px, 20px) with 12px card border radii and smooth elevation shadows.

---

## 3. Core Screen Elements & Features

### 3.1 Header & Master Power Switch
- **Brand**: Custom audio wave SVG glyph with bold `180hz nc` title and `AI Noise Suppression` subtitle.
- **Master Power Button**: Instant one-click toggle between `Active` (emerald glow) and `Bypassed` (amber glow).
- **Theme Switcher**: One-click toggle (`🌙` / `☀️`) switching between Dark and Light themes with dynamic persistence to `%APPDATA%/180hz nc/config.json`.
- **Engine Status Pill**: Glowing live indicator showing current mode status (`Krisp AI Active` / `Bypassed` / `Fallback`).

### 3.2 Microphone & Speaker Controls
- **Microphone**: Auto-detection badge, device dropdown, 0% to 200% input volume slider, and dual-purpose VU meter with calibrated dB ticks (`-60`, `-30`, `-12`, `-3`, `0 dB`).
- **`CLIP` Indicator**: Flashing red LED warning triggered immediately when input audio reaches $\ge -0.5$ dBFS.
- **Speaker / Headphones**: Auto-detection badge, device dropdown, and 0% to 100% monitor volume slider.

### 3.3 Live Voice Tester (Hero Centerpiece)
- **Central Circular Button**: Large 72px interactive microphone button with glowing emerald pulse animation when testing.
- **A/B Monitor Stream Switch**: Click to instantly toggle between `✨ Cleaned` (AI noise suppression) and `🎙️ Original` (Dry unprocessed mic audio) directly in headphones.
- **10-Second Record & Replay**:
  - `⏺ Record 10s`: Captures both raw and cleaned audio buffers simultaneously.
  - `▶ Play Replay`: Plays back the recorded 10-second snippet for evaluation.
  - `💾 Export WAV`: Exports timestamped `raw_input.wav` and `cleaned_output.wav` directly to disk.

### 3.4 Noise Suppression & DSP Telemetry
- **4-Mode Segmented Switch**: `None`, `Light`, `Standard`, and `Max Quality`.
- **Suppression Strength Slider**: Continuous 0% to 100% fader with zero dry-signal leakage at 100%.
- **Live DSP Stats**: Real-time counter of processed frames, pass-through frames (guaranteed 0), and DSP latency in microseconds (`~180 µs`).
- **`Self-Test`**: Runs synthetic audio through all 8 pipeline stages and displays a comprehensive diagnostic report modal.

### 3.5 Advanced Features
- **Auto-Adjust Input Sensitivity**: Adapts dynamically to room noise floor and vocal intensity. Includes manual threshold slider when disabled.
- **Echo Cancellation (AEC)**: Filters acoustic speaker bleed with ERLE and delay tracking. Features `🔊 Speaker Test` and `🩺 Echo Doctor` modal for diagnosing loopbacks and Discord screen share echo.
- **Push to Talk**: Customizable keyboard shortcut binding (e.g., `Space`, mouse keys) with live transmission badge.

---

## 4. Zero CPU Idle Optimization
- When minimized to tray or hidden (`document.hidden === true`):
  - Metric polling drops from 60 ms to 2500 ms.
  - CSS animations and canvas loops are paused.
  - App UI consumes **0% CPU** in the background.

---

## 5. Screenshots

- **Dark Theme (Default)**: `docs/screenshots/theme_classic_dark.png`
- **Light Theme**: `docs/screenshots/theme_classic_light.png`
- **Echo Doctor Diagnostic**: `docs/screenshots/echo_doctor_modal.png`
