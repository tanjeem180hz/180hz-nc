/**
 * 180Hz NC - Signal Lab Orb & Waveform Ring Renderer
 *
 * Renders the signature circular oscilloscope display:
 * - Outer jagged desaturated spikes: Raw ambient noise
 * - Inner glowing smooth ring: Pristine cleaned voice
 * - Radial collapse lines: Visual demonstration of noise suppression
 * - 180 Hz Notch indicator lock at 12 o'clock
 *
 * Strictly throttled to 30 fps and pauses completely (0% CPU) when window is hidden.
 */

(function () {
  const canvas = document.getElementById('signalOrbCanvas');
  if (!canvas) return;

  const ctx = canvas.getContext('2d');
  const dpr = window.devicePixelRatio || 1;
  const CSS_SIZE = 220;

  canvas.width = CSS_SIZE * dpr;
  canvas.height = CSS_SIZE * dpr;
  ctx.scale(dpr, dpr);

  const cx = CSS_SIZE / 2;
  const cy = CSS_SIZE / 2;
  const R_OUTER = 92;
  const R_INNER = 64;
  const R_HOLE = 52;

  // Waveform state (32 points, smoothed)
  let rawBuffer = new Float32Array(32);
  let cleanBuffer = new Float32Array(32);
  let targetRaw = new Float32Array(32);
  let targetClean = new Float32Array(32);

  let isSuppressionActive = true;
  let isBypassed = false;
  let isVisible = !document.hidden;
  let rafId = null;
  let lastFrameTime = 0;
  let animPhase = 0;

  // Theme palettes
  function getThemeColors() {
    const theme = document.documentElement.dataset.theme || 'night-lab';
    switch (theme) {
      case 'daylight':
        return {
          graticule: 'rgba(2, 132, 199, 0.12)',
          graticuleTicks: 'rgba(2, 132, 199, 0.20)',
          notchLock: '#0284c7',
          noiseSpikes: isBypassed ? 'rgba(217, 119, 6, 0.70)' : 'rgba(100, 116, 139, 0.45)',
          noiseCollapse: 'rgba(2, 132, 199, 0.18)',
          voiceRing: '#0284c7',
          voiceGlow: 'rgba(2, 132, 199, 0.35)',
          voiceFill: 'rgba(2, 132, 199, 0.06)',
          idleBreath: 'rgba(2, 132, 199, 0.25)',
        };
      case 'amber-console':
        return {
          graticule: 'rgba(245, 158, 11, 0.14)',
          graticuleTicks: 'rgba(245, 158, 11, 0.28)',
          notchLock: '#fbbf24',
          noiseSpikes: isBypassed ? 'rgba(239, 68, 68, 0.75)' : 'rgba(180, 83, 9, 0.55)',
          noiseCollapse: 'rgba(245, 158, 11, 0.25)',
          voiceRing: '#fbbf24',
          voiceGlow: 'rgba(245, 158, 11, 0.55)',
          voiceFill: 'rgba(245, 158, 11, 0.08)',
          idleBreath: 'rgba(245, 158, 11, 0.30)',
        };
      case 'night-lab':
      default:
        return {
          graticule: 'rgba(0, 240, 255, 0.10)',
          graticuleTicks: 'rgba(0, 240, 255, 0.22)',
          notchLock: '#00f0ff',
          noiseSpikes: isBypassed ? 'rgba(245, 158, 11, 0.75)' : 'rgba(148, 163, 184, 0.50)',
          noiseCollapse: 'rgba(0, 240, 255, 0.20)',
          voiceRing: '#00f0ff',
          voiceGlow: 'rgba(0, 240, 255, 0.60)',
          voiceFill: 'rgba(0, 240, 255, 0.08)',
          idleBreath: 'rgba(0, 240, 255, 0.35)',
        };
    }
  }

  function renderOrb() {
    ctx.clearRect(0, 0, CSS_SIZE, CSS_SIZE);
    const colors = getThemeColors();
    animPhase += 0.03;

    // Smooth interpolation toward targets (damping)
    for (let i = 0; i < 32; i++) {
      rawBuffer[i] += (targetRaw[i] - rawBuffer[i]) * 0.35;
      cleanBuffer[i] += (targetClean[i] - cleanBuffer[i]) * 0.35;
    }

    // 1. Draw Concentric Graticule Circles & Ticks
    ctx.save();
    ctx.lineWidth = 1;
    ctx.strokeStyle = colors.graticule;
    ctx.setLineDash([2, 4]);

    // -40 dB ring
    ctx.beginPath();
    ctx.arc(cx, cy, R_INNER - 4, 0, Math.PI * 2);
    ctx.stroke();

    // -20 dB ring
    ctx.beginPath();
    ctx.arc(cx, cy, (R_INNER + R_OUTER) / 2, 0, Math.PI * 2);
    ctx.stroke();

    // 0 dB outer ring
    ctx.beginPath();
    ctx.arc(cx, cy, R_OUTER, 0, Math.PI * 2);
    ctx.stroke();
    ctx.setLineDash([]);

    // 12 Radial Studio Clock Ticks
    for (let a = 0; a < 12; a++) {
      const angle = (a * Math.PI) / 6;
      const x1 = cx + Math.cos(angle) * (R_OUTER - 4);
      const y1 = cy + Math.sin(angle) * (R_OUTER - 4);
      const x2 = cx + Math.cos(angle) * (R_OUTER + 3);
      const y2 = cy + Math.sin(angle) * (R_OUTER + 3);

      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.strokeStyle = colors.graticuleTicks;
      ctx.stroke();
    }

    // 180 Hz Notch Lock Indicator at 12 o'clock (Angle -PI/2)
    const lockAngle = -Math.PI / 2;
    const lockX = cx + Math.cos(lockAngle) * (R_OUTER + 8);
    const lockY = cy + Math.sin(lockAngle) * (R_OUTER + 8);
    ctx.fillStyle = colors.notchLock;
    ctx.beginPath();
    ctx.moveTo(lockX, lockY);
    ctx.lineTo(lockX - 4, lockY - 6);
    ctx.lineTo(lockX + 4, lockY - 6);
    ctx.closePath();
    ctx.fill();

    // 2. Compute Raw and Clean Radii across 64 angular steps
    const numSteps = 64;
    const rawPoints = [];
    const cleanPoints = [];
    let rawEnergy = 0;
    let cleanEnergy = 0;

    for (let s = 0; s < numSteps; s++) {
      const theta = (s / numSteps) * Math.PI * 2 - Math.PI / 2;
      const bufIdx = Math.floor((s / numSteps) * 32);

      const rawVal = Math.abs(rawBuffer[bufIdx]);
      const cleanVal = Math.abs(cleanBuffer[bufIdx]);
      rawEnergy += rawVal;
      cleanEnergy += cleanVal;

      // Subtle idle breathing sine wave when quiet
      const breath = Math.sin(theta * 3 + animPhase) * 1.5;

      // Outer noise amplitude
      const rNoise = R_OUTER + rawVal * 28 + breath;
      const nx = cx + Math.cos(theta) * rNoise;
      const ny = cy + Math.sin(theta) * rNoise;
      rawPoints.push({ x: nx, y: ny, theta, val: rawVal });

      // Inner voice amplitude
      const rVoice = R_INNER + cleanVal * 16 + breath * 0.5;
      const vx = cx + Math.cos(theta) * rVoice;
      const vy = cy + Math.sin(theta) * rVoice;
      cleanPoints.push({ x: vx, y: vy, theta, val: cleanVal });
    }

    // 3. Draw Noise-to-Voice Radial Collapse Lines (When suppression active and noise present)
    if (isSuppressionActive && !isBypassed && rawEnergy > 0.1) {
      ctx.strokeStyle = colors.noiseCollapse;
      ctx.lineWidth = 1;
      for (let s = 0; s < numSteps; s += 4) {
        if (rawPoints[s].val > 0.02) {
          ctx.beginPath();
          ctx.moveTo(rawPoints[s].x, rawPoints[s].y);
          ctx.lineTo(cleanPoints[s].x, cleanPoints[s].y);
          ctx.stroke();
        }
      }
    }

    // 4. Draw Outer Noise Spikes
    ctx.beginPath();
    ctx.strokeStyle = colors.noiseSpikes;
    ctx.lineWidth = isBypassed ? 2 : 1.5;
    for (let s = 0; s < numSteps; s++) {
      if (s === 0) ctx.moveTo(rawPoints[s].x, rawPoints[s].y);
      else ctx.lineTo(rawPoints[s].x, rawPoints[s].y);
    }
    ctx.closePath();
    ctx.stroke();

    // 5. Draw Clean Voice Ring with Glow
    ctx.save();
    ctx.shadowColor = colors.voiceGlow;
    ctx.shadowBlur = cleanEnergy > 0.05 ? 12 : 4;
    ctx.strokeStyle = colors.voiceRing;
    ctx.lineWidth = cleanEnergy > 0.05 ? 2.5 : 1.8;

    ctx.beginPath();
    for (let s = 0; s < numSteps; s++) {
      // Cardinal spline approximation for silky smooth ring
      const p0 = cleanPoints[(s - 1 + numSteps) % numSteps];
      const p1 = cleanPoints[s];
      const p2 = cleanPoints[(s + 1) % numSteps];
      const midX = (p1.x + p2.x) / 2;
      const midY = (p1.y + p2.y) / 2;
      if (s === 0) ctx.moveTo(p1.x, p1.y);
      else ctx.quadraticCurveTo(p1.x, p1.y, midX, midY);
    }
    ctx.closePath();
    ctx.fillStyle = colors.voiceFill;
    ctx.fill();
    ctx.stroke();
    ctx.restore();

    // 6. Draw Center Clear Mask for Tactile Power Button
    ctx.save();
    ctx.beginPath();
    ctx.arc(cx, cy, R_HOLE, 0, Math.PI * 2);
    ctx.strokeStyle = colors.graticule;
    ctx.lineWidth = 1;
    ctx.stroke();
    ctx.restore();

    ctx.restore();
  }

  function loop(now) {
    if (!isVisible) return;
    rafId = requestAnimationFrame(loop);

    // Limit to 30 fps
    if (now - lastFrameTime < 32) return;
    lastFrameTime = now;

    renderOrb();
  }

  // Window visibility listener for 0% CPU consumption when hidden
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
      isVisible = false;
      if (rafId) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
    } else {
      isVisible = true;
      lastFrameTime = performance.now();
      rafId = requestAnimationFrame(loop);
    }
  });

  // Global interface to feed metrics from app.js
  window.SignalOrb = {
    update(rawArr, cleanArr, active, bypassed) {
      isSuppressionActive = !!active;
      isBypassed = !!bypassed;

      if (Array.isArray(rawArr) && rawArr.length === 32) {
        for (let i = 0; i < 32; i++) targetRaw[i] = rawArr[i];
      }
      if (Array.isArray(cleanArr) && cleanArr.length === 32) {
        for (let i = 0; i < 32; i++) targetClean[i] = cleanArr[i];
      }
    },
    triggerRedraw() {
      if (isVisible) renderOrb();
    },
  };

  // Start animation loop
  lastFrameTime = performance.now();
  rafId = requestAnimationFrame(loop);
})();
