// ==========================================================================
// 180hz nc - Pure Minimal Single-Option Controller with Live Voice Sonar
// Real-Time 360° Acoustic Radar, Voice Sonar Pings & Depth Visualizer
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
      const now = Date.now() / 180;
      const wave = Array.from({ length: 48 }, (_, i) => Math.sin(i * 0.45 + now) * 0.4);
      return Promise.resolve({
        input_peak_db: -22.0,
        output_peak_db: -24.0,
        raw_waveform: wave,
        clean_waveform: wave,
        suppression_active: true,
      });
    }
    case 'toggle_live_test':
      return Promise.resolve(true);
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

// Sonar Simulation State
let sweepAngle = 0;
const sonarPings = [];
const sonarBlips = [];
let lastPingTime = 0;

// DOM Elements
const masterToggle = document.getElementById('masterToggle');
const ambientGlow = document.getElementById('ambientGlow');
const voiceContainer = document.getElementById('voiceContainer');

// Canvas 1: 360° Circular Acoustic Sonar Radar
const radarCanvas = document.getElementById('sonarRadarCanvas');
const radarCtx = radarCanvas ? radarCanvas.getContext('2d') : null;

// Canvas 2: Live Voice Sonar Depth Waveform
const voiceCanvas = document.getElementById('voiceCanvas');
const voiceCtx = voiceCanvas ? voiceCanvas.getContext('2d') : null;

// Retina canvas scaling
function setupCanvases() {
  const dpr = window.devicePixelRatio || 1;

  if (radarCanvas && radarCtx) {
    radarCanvas.width = 340 * dpr;
    radarCanvas.height = 340 * dpr;
    radarCtx.setTransform(1, 0, 0, 1, 0, 0);
    radarCtx.scale(dpr, dpr);
  }

  if (voiceCanvas && voiceCtx) {
    voiceCanvas.width = 300 * dpr;
    voiceCanvas.height = 76 * dpr;
    voiceCtx.setTransform(1, 0, 0, 1, 0, 0);
    voiceCtx.scale(dpr, dpr);
  }
}

// Toggle master suppression state
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
  setupCanvases();
  window.addEventListener('resize', setupCanvases);

  // Master single-click toggle
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
  requestAnimationFrame(renderLoop);
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
        // Normalized 0.0 to 1.0 linear loudness
        const peakLinear = Math.max(0, Math.min(1, (peak + 58) / 58));
        smoothedPeak += (peakLinear - smoothedPeak) * 0.35;
      }
    } catch (err) {
      // Ignore transient poll error
    } finally {
      isPolling = false;
    }
  }, 35);
}

// ----------------------------------------------------------------------------
// RENDER LOOP (60 FPS)
// ----------------------------------------------------------------------------
let globalPhase = 0;

function renderLoop() {
  globalPhase += 0.05;

  // Smooth lerp waveform towards target
  for (let i = 0; i < targetWaveform.length; i++) {
    if (currentWaveform[i] === undefined) currentWaveform[i] = 0;
    currentWaveform[i] += (targetWaveform[i] - currentWaveform[i]) * 0.28;
  }

  // 1. Check for Acoustic Sonar Pings (triggered by real-time voice energy)
  const now = performance.now();
  if (smoothedPeak > 0.035 && now - lastPingTime > (240 - smoothedPeak * 120)) {
    lastPingTime = now;
    sonarPings.push({
      radius: 82,
      speed: 1.8 + smoothedPeak * 3.2,
      maxRadius: 164,
      opacity: Math.min(1.0, 0.4 + smoothedPeak * 1.5),
      wave: targetWaveform.slice(),
    });

    // Spawn harmonic contact blips along sonar beam
    if (isActive && Math.random() < 0.65) {
      const r = 94 + Math.random() * 60;
      const angle = sweepAngle - (Math.random() * 0.3);
      sonarBlips.push({
        x: 170 + Math.cos(angle) * r,
        y: 170 + Math.sin(angle) * r,
        size: 2.2 + smoothedPeak * 3.0,
        opacity: 0.95,
      });
    }
  }

  // Render both sonar scopes
  renderSonarRadar();
  renderVoiceSonarWave();

  requestAnimationFrame(renderLoop);
}

// ----------------------------------------------------------------------------
// CANVAS 1: 360° Circular Acoustic Sonar Radar
// ----------------------------------------------------------------------------
function renderSonarRadar() {
  if (!radarCtx || !radarCanvas) return;
  const ctx = radarCtx;
  const w = 340;
  const h = 340;
  const cx = 170;
  const cy = 170;

  ctx.clearRect(0, 0, w, h);

  // Rotate sweep beam
  sweepAngle = (sweepAngle + (isActive ? 0.024 : 0.012)) % (Math.PI * 2);

  // A. Concentric Sonar Distance Rings
  const rings = [92, 116, 140, 162];
  ctx.save();
  for (let i = 0; i < rings.length; i++) {
    const r = rings[i];
    ctx.beginPath();
    ctx.arc(cx, cy, r, 0, Math.PI * 2);
    ctx.strokeStyle = isActive ? 'rgba(16, 185, 129, 0.14)' : 'rgba(75, 85, 99, 0.12)';
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 6]);
    ctx.stroke();
  }

  // Crosshairs & acoustic 45° angle ticks
  ctx.setLineDash([2, 8]);
  ctx.strokeStyle = isActive ? 'rgba(16, 185, 129, 0.10)' : 'rgba(75, 85, 99, 0.08)';
  for (let deg = 0; deg < 360; deg += 45) {
    const rad = (deg * Math.PI) / 180;
    ctx.beginPath();
    ctx.moveTo(cx + Math.cos(rad) * 88, cy + Math.sin(rad) * 88);
    ctx.lineTo(cx + Math.cos(rad) * 162, cy + Math.sin(rad) * 162);
    ctx.stroke();
  }
  ctx.restore();

  // B. Rotating Sonar Sweep Beam with Phosphor Trail
  ctx.save();
  const trailSegments = 24;
  const trailSpan = 0.55; // radians of sweep cone
  for (let i = 0; i < trailSegments; i++) {
    const frac = i / trailSegments;
    const a1 = sweepAngle - (trailSpan * (1 - frac));
    const a2 = sweepAngle - (trailSpan * (1 - (i + 1) / trailSegments));
    const alpha = frac * (isActive ? 0.16 : 0.06);

    ctx.beginPath();
    ctx.moveTo(cx + Math.cos(a1) * 82, cy + Math.sin(a1) * 82);
    ctx.lineTo(cx + Math.cos(a1) * 162, cy + Math.sin(a1) * 162);
    ctx.lineTo(cx + Math.cos(a2) * 162, cy + Math.sin(a2) * 162);
    ctx.lineTo(cx + Math.cos(a2) * 82, cy + Math.sin(a2) * 82);
    ctx.closePath();
    ctx.fillStyle = isActive ? `rgba(16, 185, 129, ${alpha})` : `rgba(107, 114, 128, ${alpha})`;
    ctx.fill();
  }

  // Sweep Leading Ray
  ctx.beginPath();
  ctx.moveTo(cx + Math.cos(sweepAngle) * 82, cy + Math.sin(sweepAngle) * 82);
  ctx.lineTo(cx + Math.cos(sweepAngle) * 163, cy + Math.sin(sweepAngle) * 163);
  ctx.strokeStyle = isActive ? 'rgba(52, 211, 153, 0.85)' : 'rgba(156, 163, 175, 0.4)';
  ctx.lineWidth = 1.6;
  if (isActive) {
    ctx.shadowColor = '#00ff9d';
    ctx.shadowBlur = 8;
  }
  ctx.stroke();
  ctx.restore();

  // C. Expanding Acoustic Voice Sonar Shockwaves (Pings)
  for (let p = sonarPings.length - 1; p >= 0; p--) {
    const ping = sonarPings[p];
    ping.radius += ping.speed;
    ping.opacity *= 0.952;

    if (ping.radius > ping.maxRadius || ping.opacity < 0.015) {
      sonarPings.splice(p, 1);
      continue;
    }

    ctx.save();
    ctx.beginPath();
    const steps = 64;
    for (let i = 0; i <= steps; i++) {
      const phi = (i / steps) * Math.PI * 2;
      const waveIdx = Math.floor((i / steps) * (ping.wave.length || 48)) % (ping.wave.length || 48);
      const wavePerturb = (ping.wave[waveIdx] || 0) * (14 * smoothedPeak);
      const r = ping.radius + wavePerturb;
      const x = cx + Math.cos(phi) * r;
      const y = cy + Math.sin(phi) * r;

      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    }
    ctx.closePath();

    if (isActive) {
      ctx.strokeStyle = `rgba(16, 185, 129, ${ping.opacity})`;
      ctx.shadowColor = 'rgba(0, 255, 157, 0.7)';
      ctx.shadowBlur = 10;
      ctx.lineWidth = 1.8;
    } else {
      ctx.strokeStyle = `rgba(107, 114, 128, ${ping.opacity * 0.4})`;
      ctx.shadowBlur = 0;
      ctx.lineWidth = 1.2;
    }
    ctx.stroke();
    ctx.restore();
  }

  // D. Sonar Contact Blips (Harmonics)
  for (let b = sonarBlips.length - 1; b >= 0; b--) {
    const blip = sonarBlips[b];
    blip.opacity *= 0.965;

    if (blip.opacity < 0.02) {
      sonarBlips.splice(b, 1);
      continue;
    }

    ctx.save();
    ctx.beginPath();
    ctx.arc(blip.x, blip.y, blip.size, 0, Math.PI * 2);
    ctx.fillStyle = isActive ? `rgba(52, 211, 153, ${blip.opacity})` : `rgba(156, 163, 175, ${blip.opacity * 0.4})`;
    if (isActive) {
      ctx.shadowColor = '#10b981';
      ctx.shadowBlur = 12;
    }
    ctx.fill();
    ctx.restore();
  }
}

// ----------------------------------------------------------------------------
// CANVAS 2: Live Voice Sonar Depth / Waveform Visualizer
// ----------------------------------------------------------------------------
function renderVoiceSonarWave() {
  if (!voiceCtx || !voiceCanvas) return;
  const ctx = voiceCtx;
  const w = 300;
  const h = 76;
  const cy = h / 2;

  ctx.clearRect(0, 0, w, h);

  // Background Sonar Grid (Depth reference lines)
  ctx.save();
  ctx.strokeStyle = isActive ? 'rgba(16, 185, 129, 0.08)' : 'rgba(75, 85, 99, 0.07)';
  ctx.lineWidth = 1;
  ctx.setLineDash([3, 5]);

  ctx.beginPath();
  ctx.moveTo(12, cy - 18);
  ctx.lineTo(w - 12, cy - 18);
  ctx.moveTo(12, cy);
  ctx.lineTo(w - 12, cy);
  ctx.moveTo(12, cy + 18);
  ctx.lineTo(w - 12, cy + 18);
  ctx.stroke();

  // Subtle vertical sonar range markers
  for (let x = 30; x < w; x += 48) {
    ctx.beginPath();
    ctx.moveTo(x, cy - 6);
    ctx.lineTo(x, cy + 6);
    ctx.stroke();
  }
  ctx.restore();

  const points = currentWaveform.length || 48;
  const step = (w - 24) / (points - 1);
  const startX = 12;

  // Upper Waveform Coordinates
  const upperPts = [];
  const lowerPts = [];

  for (let i = 0; i < points; i++) {
    const x = startX + i * step;
    const waveVal = currentWaveform[i] || 0;
    const breath = Math.sin(globalPhase + i * 0.22) * 1.6;
    const amp = waveVal * (h * 0.42) + (smoothedPeak > 0.04 ? breath * (1 + smoothedPeak * 3.5) : breath * 0.5);

    upperPts.push({ x, y: cy - amp });
    lowerPts.push({ x, y: cy + amp });
  }

  // Draw Filled Symmetrical Acoustic Sonar Envelope
  ctx.save();
  ctx.beginPath();
  ctx.moveTo(upperPts[0].x, upperPts[0].y);
  for (let i = 1; i < points; i++) {
    const prev = upperPts[i - 1];
    const curr = upperPts[i];
    const cpX = (prev.x + curr.x) / 2;
    ctx.bezierCurveTo(cpX, prev.y, cpX, curr.y, curr.x, curr.y);
  }
  for (let i = points - 1; i >= 0; i--) {
    const curr = lowerPts[i];
    ctx.lineTo(curr.x, curr.y);
  }
  ctx.closePath();

  const fillGrad = ctx.createLinearGradient(0, 0, 0, h);
  if (isActive) {
    fillGrad.addColorStop(0, 'rgba(16, 185, 129, 0.22)');
    fillGrad.addColorStop(0.5, 'rgba(6, 182, 212, 0.08)');
    fillGrad.addColorStop(1, 'rgba(16, 185, 129, 0.22)');
  } else {
    fillGrad.addColorStop(0, 'rgba(75, 85, 99, 0.15)');
    fillGrad.addColorStop(0.5, 'rgba(55, 65, 81, 0.03)');
    fillGrad.addColorStop(1, 'rgba(75, 85, 99, 0.15)');
  }
  ctx.fillStyle = fillGrad;
  ctx.fill();

  // Draw Upper Sonar Wave Line
  ctx.beginPath();
  ctx.moveTo(upperPts[0].x, upperPts[0].y);
  for (let i = 1; i < points; i++) {
    const prev = upperPts[i - 1];
    const curr = upperPts[i];
    const cpX = (prev.x + curr.x) / 2;
    ctx.bezierCurveTo(cpX, prev.y, cpX, curr.y, curr.x, curr.y);
  }
  ctx.strokeStyle = isActive ? '#10b981' : '#4b5563';
  ctx.lineWidth = 2.2;
  if (isActive) {
    ctx.shadowColor = 'rgba(16, 185, 129, 0.75)';
    ctx.shadowBlur = 10;
  }
  ctx.stroke();

  // Draw Lower Sonar Wave Line
  ctx.beginPath();
  ctx.moveTo(lowerPts[0].x, lowerPts[0].y);
  for (let i = 1; i < points; i++) {
    const prev = lowerPts[i - 1];
    const curr = lowerPts[i];
    const cpX = (prev.x + curr.x) / 2;
    ctx.bezierCurveTo(cpX, prev.y, cpX, curr.y, curr.x, curr.y);
  }
  ctx.strokeStyle = isActive ? 'rgba(6, 182, 212, 0.85)' : '#374151';
  ctx.lineWidth = 1.8;
  if (isActive) {
    ctx.shadowColor = 'rgba(6, 182, 212, 0.6)';
    ctx.shadowBlur = 8;
  }
  ctx.stroke();

  // Pulse Dot at Peak Voice Coordinate
  if (isActive && smoothedPeak > 0.06) {
    const maxIdx = Math.floor(points / 2);
    const dotX = upperPts[maxIdx].x;
    const dotY = upperPts[maxIdx].y;
    ctx.beginPath();
    ctx.arc(dotX, dotY, 3.2, 0, Math.PI * 2);
    ctx.fillStyle = '#00ff9d';
    ctx.shadowColor = '#00ff9d';
    ctx.shadowBlur = 12;
    ctx.fill();
  }

  ctx.restore();
}

// Launch on DOM ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
