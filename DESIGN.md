# 180hz nc UI Design System: Soft Glass Architecture

## 1. Concept & Philosophy
**180hz nc** features the **"Soft Glass"** desktop workspace theme — a modern, tactile interface with subtle translucent panels, soft frosted-glass surfaces, gentle blur effects, and disciplined rounded corners (12px for cards, 8px for controls and inputs).

The design uses a calm pastel-tinted gradient backdrop, low-glare neutral text, and soft accent highlights so dense, professional audio tools feel calm, focused, and uncluttered.

---

## 2. Design Tokens & Visual Hierarchy (`theme-soft-glass.css`)

All design variables and theme tokens are centralized in [`theme-soft-glass.css`](file:///e:/GitHubProject/NoiseCancelator/crates/quietmic-app/ui/theme-soft-glass.css).

### 2.1 Spacing & Radius Scale
Consistent 4/8 px scale ensures predictable rhythm and generous padding across all DPIs:
- **Spacing Grid**:
  - `--space-1`: `4px`
  - `--space-2`: `8px`
  - `--space-3`: `12px`
  - `--space-4`: `16px`
  - `--space-5`: `20px`
  - `--space-6`: `24px`
  - `--space-8`: `32px`
- **Shape Scale**:
  - `--radius-card`: `12px` (Cards, panels, modal containers)
  - `--radius-control`: `8px` (Buttons, inputs, selects, sliders, segmented switches)
  - `--radius-sm`: `6px` (Inner buttons, badges)
  - `--radius-pill`: `9999px` (Status indicators, toggle track, VU meters)

### 2.2 Color Tokens & Contrast Matrix

| Category | Soft Glass Dark (Default) | Soft Glass Light (`daylight`) | Reduce Effects Mode |
| :--- | :--- | :--- | :--- |
| **Backdrop Gradient** | `linear-gradient(145deg, #0e121a 0%, #151a28 50%, #111e22 100%)` | `linear-gradient(135deg, #e9ecf8 0%, #dfe9f4 50%, #e0f2ec 100%)` | Fixed solid tint |
| **Panel Surface** | `rgba(22, 28, 42, 0.65)` | `rgba(255, 255, 255, 0.62)` | `#19202f` (Dark) / `#eaf0f7` (Light) |
| **Hairline Border** | `1px solid rgba(255, 255, 255, 0.12)` | `1px solid rgba(255, 255, 255, 0.70)` | `rgba(255, 255, 255, 0.10)` / `#ccd6e4` |
| **Backdrop Filter** | `blur(12px) saturate(120%)` | `blur(12px) saturate(120%)` | `none` (0 ms GPU shader cost) |
| **Text Primary** | `#f0f3f8` (Soft off-white) | `#212636` (Soft charcoal) | Identical |
| **Text Secondary** | `#9da6b9` (Calm slate) | `#535d74` (Mid-tone charcoal) | Identical |
| **Accent Primary** | `#7c8cf8` (Soft Indigo) | `#5f70e5` (Deepened Indigo) | Identical |
| **Accent Secondary** | `#5ecbb0` (Soft Mint) | `#0d9674` (Emerald Green) | Identical |
| **Warning / Bypass** | `#f4be62` (Warm Amber) | `#b37302` (Deep Amber) | Identical |
| **Error / Alert** | `#f28585` (Soft Coral) | `#d74343` (Deep Coral) | Identical |
| **Clipping LED** | `#ef5350` (Vivid Red) | `#cc2b2b` (Vivid Red) | Identical |

#### Contrast Ratio Audit (WCAG AA Compliance $\ge 4.5:1$):
- **Light Theme**:
  - Primary body text `#212636` on rendered card `#f5f6fb`: **13.38 : 1** (Exceeds WCAG AAA).
  - Secondary label text `#535d74` on rendered card `#f5f6fb`: **5.93 : 1** (Exceeds WCAG AA).
- **Dark Theme**:
  - Primary body text `#f0f3f8` on rendered card `#141926`: **15.65 : 1** (Exceeds WCAG AAA).
  - Secondary label text `#9da6b9` on rendered card `#141926`: **7.14 : 1** (Exceeds WCAG AAA).

---

## 3. Typography: Bundled Local Fonts & Tabular Numerals
- **Font Stack**: Bundled local **Inter** font family (weights 400, 500, 600, 700) located in [`ui/fonts/`](file:///e:/GitHubProject/NoiseCancelator/crates/quietmic-app/ui/fonts).
- **License**: SIL Open Font License, Version 1.1 (OFL-1.1) documented in [`THIRD_PARTY_NOTICES.md`](file:///e:/GitHubProject/NoiseCancelator/THIRD_PARTY_NOTICES.md).
- **Zero External Calls**: Zero Google Fonts / CDN dependencies, zero network requests, zero telemetry.
- **Tabular Numerals**: Enforced via `font-variant-numeric: tabular-nums;` on all dB scales, microsecond latency readouts, and VU meters to eliminate layout jitter.

---

## 4. Performance Rules for Glass Effects (Strict Hardware Budgets)

1. **Max 4 Blurred Surfaces**: Blur is applied strictly to large structural cards and active modals. It is never applied to nested elements, list items, chips, or inputs.
2. **Zero Animated Blur / Gradients**: All backdrops and glass filters remain static. No expensive GPU layout redraw loops.
3. **0% CPU Idle Throttling**:
   - Monitored via `document.addEventListener('visibilitychange', ...)`.
   - When the window is hidden or minimized to tray, metric polling is completely halted (`clearInterval(pollIntervalId)`).
   - Audio DSP thread continues at real-time priority in background with **0% UI CPU overhead**.
4. **Meter Polling Rate**: Calibrated to **15 fps (66 ms)**, completely eliminating CPU thrashing.
5. **Setting "Reduce effects"**:
   - Replaces all translucency and `backdrop-filter: blur(12px)` with solid tinted surfaces.
   - User controllable via toggle in the *Preferences* tab.
   - **Automatic Fallback Triggers**:
     - Automatically enables if `backdrop-filter` is unsupported.
     - Automatically enables if software GPU rendering (e.g., SwiftShader, LLVMpipe, Microsoft Basic Render) is detected via WebGL unmasked renderer.
     - Automatically enables if measured UI frame time exceeds 24 ms for $>2$ seconds continuously while visible.
     - Informs the user once via status toast upon auto-activation.

---

## 5. Navigation & Component Architecture

### 5.1 Tab Navigation
- **Audio Tab**: Primary workspace holding Microphone selector, Speaker selector, Live Voice Monitor, Noise Suppression (4-mode switch + slider), Auto-Sensitivity, Echo Cancellation, and Push-to-Talk.
- **Diagnostics Tab**: System health center holding Echo Doctor, DSP Pipeline Self-Test, AEC Speaker Calibration, and VB-CABLE routing guidance.
- **Preferences Tab**: Desktop integration controls including *Minimize to tray on close*, *Start with Windows*, *Reduce effects* toggle, and *Taskbar Visibility Pinning Guide*.
- **About Tab**: Version telemetry, DSP technical specifications, zero-latency guarantees, and OFL license notices.

### 5.2 Accessible Interactive States
- **Hover**: 150 ms ease transition with subtle surface brighten.
- **Active / Pressed**: 0.96 scale down on tactile switches.
- **Focus-Visible**: 2px high-visibility accent focus ring with 2px offset.
- **Reduced Motion**: Respects `prefers-reduced-motion: reduce` by setting transition times to 0.01 ms.

---

## 6. How to Add a New Theme

To add a new theme:
1. Define a CSS selector `[data-theme="my-theme"]` in `theme-soft-glass.css`.
2. Override the following core tokens:
   - `--bg-gradient`: Linear or radial backdrop.
   - `--glass-bg`: Card surface translucent color (`rgba(..., 0.55-0.70)`).
   - `--glass-border`: Hairline border with white/neutral tint.
   - `--text-primary` and `--text-secondary`: Ensure contrast ratio $\ge 4.5:1$.
   - `--accent-primary` and `--accent-secondary`: Key interaction colors.
3. Register the theme name in `ipc-contract.json` under `commands.set_theme.allowed`.
4. Run `cargo test -p quietmic-core --test test_ipc_contract_verification` to confirm contract integrity.

---

## 7. Screenshots & DPI Verification

| Screen State | Preview Link |
| :--- | :--- |
| **Soft Glass Dark (100% DPI)** | [theme_soft_glass_dark_100.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/theme_soft_glass_dark_100.png) |
| **Soft Glass Light (100% DPI)** | [theme_soft_glass_light_100.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/theme_soft_glass_light_100.png) |
| **Soft Glass Dark (150% DPI)** | [theme_soft_glass_dark_150.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/theme_soft_glass_dark_150.png) |
| **Soft Glass Light (150% DPI)** | [theme_soft_glass_light_150.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/theme_soft_glass_light_150.png) |
| **Soft Glass Dark (200% DPI)** | [theme_soft_glass_dark_200.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/theme_soft_glass_dark_200.png) |
| **Soft Glass Light (200% DPI)** | [theme_soft_glass_light_200.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/theme_soft_glass_light_200.png) |
| **Reduce Effects Mode (Dark)** | [theme_reduce_effects_dark.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/theme_reduce_effects_dark.png) |
| **Reduce Effects Mode (Light)** | [theme_reduce_effects_light.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/theme_reduce_effects_light.png) |
| **Diagnostics Tab** | [tab_diagnostics_dark.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/tab_diagnostics_dark.png) |
| **Preferences Tab** | [tab_preferences_dark.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/tab_preferences_dark.png) |
| **Echo Doctor Diagnostic Modal** | [modal_echo_doctor_dark.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/modal_echo_doctor_dark.png) |
| **DSP Self-Test Modal** | [modal_selftest_dark.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/modal_selftest_dark.png) |
<<<<<<< Updated upstream
| **DSP Self-Test Modal2** | [modal_selftest_dark.png](file:///e:/GitHubProject/NoiseCancelator/docs/screenshots/modal_selftest_dark.png) |
=======

That's All....
>>>>>>> Stashed changes
