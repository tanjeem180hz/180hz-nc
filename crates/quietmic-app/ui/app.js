// ==========================================================================
// 180hz nc - Pure Minimal Single-Option Controller
// Maximum AI Voice Isolation & Real-Time Voice Visualizer
// Strict adherence to ipc-contract.json & zero UI text clutter
// ==========================================================================

function getInvoke() {
  if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
    return window.__TAURI__.core.invoke;
  }
  if (window.__TAURI__ && typeof window.__TAURI__.invoke === 'function') {
    return window.__TAURI__.invoke;
  }
  return mockInvoke;
}

async function invoke(cmd, args) {
  const invoker = getInvoke();
  return invoker(cmd, args);
}

// Mock fallback for browser dev / testing
function mockInvoke(cmd, args) {
  switch (cmd) {
    case 'get_config':
      return Promise.resolve({
        mode: 'MaxQuality',
        strength: 1.0,
        bypass: false,
        auto_sensitivity: true,
      });
    case 'get_metrics': {
      const now = Date.now() / 200;
      const wave = Array.from({ length: 48 }, (_, i) => Math.sin(i * 0.4 + now) * 0.4);
      return Promise.resolve({
        input_peak_db: -24.0,
        output_peak_db: -26.0,
        raw_waveform: wave,
        clean_waveform: wave,
        suppression_active: true,
      });
    }
    case 'set_mode':
    case 'set_strength':
    case 'set_bypass':
    case 'set_auto_sensitivity':
      return Promise.resolve();
    default:
      return Promise.resolve();
  }
}

// State
let isActive = true;
let currentWaveform = new Array(48).fill(0);
let targetWaveform = new Array(48).fill(0);
let smoothedPeak = 0;

// DOM Elements
const masterToggle = document.getElementById('masterToggle');
const ambientGlow = document.getElementById('ambientGlow');
const canvas = document.getElementById('voiceCanvas');
const ctx = canvas ? canvas.getContext('2d') : null;

// Retina canvas scaling
function setupCanvas() {
  if (!canvas || !ctx) return;
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = (rect.width || 300) * dpr;
  canvas.height = (rect.height || 80) * dpr;
  ctx.scale(dpr, dpr);
}

// Toggle master state
async function setSuppressionState(active) {
  isActive = active;

  if (isActive) {
    masterToggle.classList.remove('off');
    masterToggle.classList.add('active');
    ambientGlow.classList.remove('off');

    try {
      await invoke('set_mode', { mode: 'MaxQuality' });
      await invoke('set_strength', { strength: 1.0 });
      await invoke('set_auto_sensitivity', { enabled: true });
      await invoke('set_bypass', { bypass: false });
    } catch (err) {
      console.error('Error enabling maximum suppression:', err);
    }
  } else {
    masterToggle.classList.add('off');
    masterToggle.classList.remove('active');
    ambientGlow.classList.add('off');

    try {
      await invoke('set_bypass', { bypass: true });
    } catch (err) {
      console.error('Error bypassing suppression:', err);
    }
  }
}

// Initialize
async function init() {
  setupCanvas();
  window.addEventListener('resize', setupCanvas);

  masterToggle.addEventListener('click', () => {
    setSuppressionState(!isActive);
  });

  try {
    const config = await invoke('get_config');
    if (config) {
      if (config.bypass === true) {
        setSuppressionState(false);
      } else {
        setSuppressionState(true);
      }
    }
  } catch (err) {
    console.warn('Could not fetch initial config, defaulting to Active MaxQuality:', err);
  }

  // Start polling metrics and drawing loop
  startMetricsPolling();
  requestAnimationFrame(renderVoiceLoop);
}

// Polling loop for live metrics (waveform & peak)
let isPolling = false;
function startMetricsPolling() {
  setInterval(async () => {
    if (isPolling) return;
    isPolling = true;
    try {
      const metrics = await invoke('get_metrics');
      if (metrics) {
        const wave = (isActive ? metrics.clean_waveform : metrics.raw_waveform) || [];
        if (wave.length > 0) {
          targetWaveform = wave;
        }
        const peak = Math.max(metrics.output_peak_db || -90, metrics.input_peak_db || -90);
        // Linear scale normalized roughly between 0.0 and 1.0
        const peakLinear = Math.max(0, Math.min(1, (peak + 60) / 60));
        smoothedPeak += (peakLinear - smoothedPeak) * 0.3;
      }
    } catch (err) {
      // Ignore transient poll error
    } finally {
      isPolling = false;
    }
  }, 40);
}

// Render dynamic live voice visualizer
let phase = 0;
function renderVoiceLoop() {
  if (ctx && canvas) {
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.width / dpr;
    const h = canvas.height / dpr;
    const cy = h / 2;

    ctx.clearRect(0, 0, w, h);

    // Smooth lerp waveform towards target
    phase += 0.06;
    for (let i = 0; i < targetWaveform.length; i++) {
      if (currentWaveform[i] === undefined) currentWaveform[i] = 0;
      currentWaveform[i] += (targetWaveform[i] - currentWaveform[i]) * 0.25;
    }

    const points = currentWaveform.length || 48;
    const step = w / (points - 1);

    // Styling based on active state
    if (isActive) {
      ctx.strokeStyle = '#10b981';
      ctx.shadowColor = 'rgba(16, 185, 129, 0.65)';
      ctx.shadowBlur = 12;
      ctx.lineWidth = 2.5;
    } else {
      ctx.strokeStyle = '#4b5563';
      ctx.shadowColor = 'transparent';
      ctx.shadowBlur = 0;
      ctx.lineWidth = 1.8;
    }

    // Draw primary live voice wave
    ctx.beginPath();
    for (let i = 0; i < points; i++) {
      const x = i * step;
      // Combine mic waveform with dynamic amplitude and subtle breathing floor
      const waveVal = currentWaveform[i] || 0;
      const breath = Math.sin(phase + i * 0.2) * 1.5;
      const amp = waveVal * (h * 0.42) + (smoothedPeak > 0.05 ? breath * (1 + smoothedPeak * 3) : breath * 0.5);
      const y = cy + amp;

      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        const prevX = (i - 1) * step;
        const cpX = (prevX + x) / 2;
        ctx.bezierCurveTo(cpX, cy, cpX, y, x, y);
      }
    }
    ctx.stroke();

    // Draw secondary soft harmonic wave when active
    if (isActive && smoothedPeak > 0.08) {
      ctx.strokeStyle = 'rgba(6, 182, 212, 0.45)';
      ctx.shadowColor = 'rgba(6, 182, 212, 0.4)';
      ctx.shadowBlur = 8;
      ctx.lineWidth = 1.5;

      ctx.beginPath();
      for (let i = 0; i < points; i++) {
        const x = i * step;
        const waveVal = currentWaveform[i] || 0;
        const ripple = Math.sin(phase * 1.4 + i * 0.3) * (smoothedPeak * 8);
        const y = cy - (waveVal * (h * 0.3) + ripple);

        if (i === 0) {
          ctx.moveTo(x, y);
        } else {
          const prevX = (i - 1) * step;
          const cpX = (prevX + x) / 2;
          ctx.bezierCurveTo(cpX, cy, cpX, y, x, y);
        }
      }
      ctx.stroke();
    }
  }

  requestAnimationFrame(renderVoiceLoop);
}

// Launch on DOM ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
