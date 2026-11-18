# 180 Hz Signal Lab Design System & UI Architecture

## 1. Concept & Identity
**180hz nc** is designed as the **"180 Hz Signal Lab"** — a calm, precision studio audio instrument where physical hardware meets a futuristic oscilloscope.

The name refers to **180 Hz**, the acoustic sweet spot of human speech fundamentals and the anchor frequency for speech-noise separation.

---

## 2. Signature Visual Elements

### 2.1 Frequency-Scale Ruler & 180 Hz Notch Lock
Spanning across the top edge of the instrument is a calibrated frequency scale:
`20 Hz | 60 Hz | 120 Hz | [▼ 180 Hz NOTCH LOCK] | 500 Hz | 1 kHz | 4 kHz | 10 kHz | 20 kHz`
The `180 Hz` frequency is illuminated with an indicator lock badge, anchoring the signal processing pipeline.

### 2.2 Central Signal Orb & Waveform Ring (Oscilloscope Canvas)
- **Outer Noise Spikes**: Rendered from the raw input waveform snapshot. Unwanted ambient noise (mechanical keyboard switches, fans, AC hum) appears as jagged, desaturated amber/slate spikes.
- **Inner Voice Ring**: Rendered from the cleaned output waveform snapshot. Pristine human voice forms a smooth, glowing electric cyan ring.
- **Radial Dynamic Collapse**: When noise suppression is active, outer jagged spikes are pulled inward toward the clean voice ring, visually demonstrating noise cancellation in real-time.
- **Idle Breathing**: A subtle, zero-overhead sinusoidal modulation keeps the ring alive during silent monitoring.

### 2.3 Tactile Master Power Button
Nestled directly in the center hole of the Signal Orb:
- **Active State**: Neon cyan glow with inner drop-shadow, illuminated power symbol, labeled `ACTIVE`.
- **Bypass State**: Warm amber glow, labeled `BYPASS`.
- **Error State**: Crimson red pulse, labeled `ERROR`.
- Satisfying tactile press physics (`transform: scale(0.96)`) with instant one-click bypass switching.

### 2.4 Hardware 4-Mode Segmented Switch
Tactile studio enclosure with 4 selectable DSP operating profiles:
1. **None**: True bit-exact linear pass-through (0.00 dB delta, 0% CPU).
2. **Light**: RNNoise 48 kHz recurrent neural network (ultra-low CPU for older hardware).
3. **Standard**: DeepFilterNet3 fine spectral Wiener filter with per-bin harmonic tracking.
4. **Max Quality**: Krisp-level full-bandwidth deep voice isolation.

### 2.5 Studio Strength Fader & VU Meter
- **Suppression Fader**: Horizontal slider with physical fader thumb and calibrated dB scale markings: `0 dB (0%)`, `-6 dB`, `-12 dB`, `-24 dB`, `-∞ dB (100%)`.
- **Dual-Segment Studio VU Meter**: Calibrated from `-60 dB` to `0 dB` with a white peak hold indicator bar (decaying after 15 frames) and a flashing red **`CLIP`** LED overload alert triggered at $\ge -1.0$ dBFS.

---

## 3. The 3 Studio Console Themes

| Theme | Identifier | Visual Character | Accent Primary | Background Base |
| :--- | :--- | :--- | :--- | :--- |
| **Night Lab** | `night-lab` | Modern dark studio hardware (Default) | Electric Cyan (`#00f0ff`) | Obsidian Slate (`#090a0f`) |
| **Daylight** | `daylight` | Clean precision light console | Deep Sapphire (`#0284c7`) | Aluminum White (`#eef2f6`) |
| **Amber Console** | `amber-console` | Vintage warm phosphor oscilloscope | Radiant Amber (`#f59e0b`) | Charcoal Bronze (`#0d0b07`) |

Theme preference is persisted automatically to `%APPDATA%/180hz nc/config.json` and syncs dynamically across sessions.

---

## 4. Modular Tabbed Layout

- **`01 LAB`**: Main hero dashboard. Top frequency ruler, central Signal Orb, Master Power button, 4-mode segmented switch, suppression strength fader, and real-time engine telemetry (frames processed, pass-through, DSP latency in µs).
- **`02 DEVICES`**: Microphone selection with `Auto Detected` badge, input gain fader, studio VU meter with `CLIP` LED; speaker/headphones selection with auto-detection; and VB-CABLE virtual cable status card.
- **`03 STUDIO`**: Live Voice Monitor (headphone test), A/B clickless 15 ms crossfade (`Raw` vs `Cleaned`), 10-second test audio recorder with instant replay and WAV export, auto-adjust sensitivity dynamic gate, and Push-to-Talk shortcut binding.
- **`04 DIAGNOSTICS`**: Echo Cancellation (AEC) telemetry, `🩺 Echo Doctor` one-click routing diagnostic modal with Discord screen sharing remediation, `🔊 Speaker Test`, `⚙️ Self-Test`, and `📊 Pipeline WAVs` multi-stage dumper.

---

## 5. Performance & Idle Optimization (0% CPU)

- **Zero CPU Idle in Tray**: When the window is hidden or minimized to the system tray (`document.hidden === true`), all `requestAnimationFrame` loops and telemetry polling timers are cancelled. The UI consumes **0% CPU**.
- **30 FPS Framerate Cap**: Oscilloscope rendering is throttled to 30 fps (33 ms delta lock) to ensure the UI stays silky smooth while using $< 1.5\%$ GPU on integrated Intel/AMD graphics.
- **Lock-Free Atomic Telemetry**: Telemetry and waveform snapshots (`WAVEFORM_SNAPSHOT_LEN = 32`) in Rust use atomic integer bitwise transfers (`AtomicU32`), ensuring the audio DSP thread never blocks, allocates, or locks.
- **Ultra-Lightweight UI Bundle**: Pure vanilla HTML5 + CSS3 + Canvas. Zero heavy frontend framework dependencies (bundle size $< 50$ KB total).

---

## 6. Screenshots

- **Night Lab Hero**: `docs/screenshots/theme_night_lab.png`
- **Daylight Console**: `docs/screenshots/theme_daylight.png`
- **Amber Phosphor Console**: `docs/screenshots/theme_amber_console.png`
- **Devices & Studio VU Meters**: `docs/screenshots/tab_devices.png`
- **Studio & Live Voice Monitor**: `docs/screenshots/tab_studio.png`
- **Diagnostics & Echo Doctor**: `docs/screenshots/tab_diagnostics.png`
