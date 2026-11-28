// ==========================================================================
// 180hz nc - Isolated Live Mic Testing Module
// Temporary module for listening to your own microphone in real time (sidetone).
// Self-contained in this single file so it can be cleanly removed later once
// the backend is fully validated.
// ==========================================================================

(function () {
  function getInvoke() {
    if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
      return window.__TAURI__.core.invoke;
    }
    if (window.__TAURI__ && typeof window.__TAURI__.invoke === 'function') {
      return window.__TAURI__.invoke;
    }
    return null;
  }

  // 1. Inject isolated stylesheet
  const style = document.createElement('style');
  style.id = 'live-mic-isolated-styles';
  style.textContent = `
    .live-mic-toggle-btn {
      position: absolute;
      top: 18px;
      right: 18px;
      width: 44px;
      height: 44px;
      border-radius: 50%;
      background: rgba(18, 24, 38, 0.78);
      border: 1px solid rgba(255, 255, 255, 0.08);
      display: flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      z-index: 100;
      transition: all 0.28s cubic-bezier(0.34, 1.56, 0.64, 1);
      box-shadow: 0 4px 14px rgba(0, 0, 0, 0.45);
      backdrop-filter: blur(14px);
      -webkit-tap-highlight-color: transparent;
      outline: none;
    }
    .live-mic-toggle-btn:hover {
      transform: scale(1.08);
      border-color: rgba(6, 182, 212, 0.45);
      box-shadow: 0 4px 18px rgba(0, 0, 0, 0.55), 0 0 12px rgba(6, 182, 212, 0.3);
    }
    .live-mic-toggle-btn:active {
      transform: scale(0.92);
    }
    .live-mic-toggle-btn.active {
      background: linear-gradient(135deg, rgba(6, 182, 212, 0.28), rgba(16, 185, 129, 0.22));
      border-color: rgba(6, 182, 212, 0.9);
      box-shadow: 0 0 18px rgba(6, 182, 212, 0.6), inset 0 0 10px rgba(6, 182, 212, 0.35);
    }
    .live-mic-svg {
      width: 22px;
      height: 22px;
      color: #6b7280;
      transition: color 0.25s ease, filter 0.25s ease;
    }
    .live-mic-toggle-btn.active .live-mic-svg {
      color: #00f0ff;
      filter: drop-shadow(0 0 6px rgba(0, 240, 255, 0.85));
    }
    .live-mic-wave-pulse {
      position: absolute;
      inset: -6px;
      border-radius: 50%;
      border: 1.5px solid rgba(6, 182, 212, 0.5);
      opacity: 0;
      pointer-events: none;
    }
    .live-mic-toggle-btn.active .live-mic-wave-pulse {
      animation: liveMicPulseAnim 2.2s infinite ease-out;
    }
    .aec-toggle-btn {
      top: 72px;
      right: 18px;
    }
    .aec-toggle-btn.active {
      background: linear-gradient(135deg, rgba(16, 185, 129, 0.28), rgba(6, 182, 212, 0.22));
      border-color: rgba(16, 185, 129, 0.9);
      box-shadow: 0 0 18px rgba(16, 185, 129, 0.55), inset 0 0 10px rgba(16, 185, 129, 0.30);
    }
    .aec-toggle-btn.active .live-mic-svg {
      color: #34d399;
      filter: drop-shadow(0 0 6px rgba(52, 211, 153, 0.85));
    }
    .aec-toggle-btn .live-mic-wave-pulse {
      border-color: rgba(16, 185, 129, 0.5);
    }
    .aec-off-slash {
      opacity: 0.92;
      transition: opacity 0.2s ease;
    }
    .aec-toggle-btn.active .aec-off-slash {
      opacity: 0;
    }
    @keyframes liveMicPulseAnim {
      0% { transform: scale(0.95); opacity: 0.8; }
      50% { transform: scale(1.24); opacity: 0.18; }
      100% { transform: scale(1.4); opacity: 0; }
    }
  `;
  document.head.appendChild(style);

  // 2. Inject isolated Live Mic button
  const liveMicBtn = document.createElement('button');
  liveMicBtn.type = 'button';
  liveMicBtn.id = 'liveMicBtn';
  liveMicBtn.className = 'live-mic-toggle-btn';
  liveMicBtn.setAttribute('aria-label', 'Toggle Live Mic');

  liveMicBtn.innerHTML = `
    <div class="live-mic-wave-pulse"></div>
    <svg class="live-mic-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M3 18v-6a9 9 0 0 1 18 0v6"></path>
      <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"></path>
    </svg>
  `;

  document.body.appendChild(liveMicBtn);

  // 2b. Inject Echo Cancellation Toggle button directly below Live Mic button
  const aecToggleBtn = document.createElement('button');
  aecToggleBtn.type = 'button';
  aecToggleBtn.id = 'aecToggleBtn';
  aecToggleBtn.className = 'live-mic-toggle-btn aec-toggle-btn active';
  aecToggleBtn.setAttribute('aria-label', 'Toggle Echo Cancellation');
  aecToggleBtn.title = 'Echo Cancellation';

  aecToggleBtn.innerHTML = `
    <div class="live-mic-wave-pulse"></div>
    <svg class="live-mic-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon>
      <path d="M15.54 8.46a5 5 0 0 1 0 7.07"></path>
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14"></path>
      <line class="aec-off-slash" x1="22" y1="2" x2="2" y2="22"></line>
    </svg>
  `;

  document.body.appendChild(aecToggleBtn);

  // 3. State & Toggle Logic
  let isLiveMicActive = false;
  let isAecActive = true;
  let aecSyncIgnoreUntil = 0;

  async function toggleLiveMic() {
    const invoker = getInvoke();
    if (!invoker) {
      // Mock for browser testing
      isLiveMicActive = !isLiveMicActive;
      updateButtonState(isLiveMicActive);
      return;
    }

    try {
      const active = await invoker('toggle_live_test', { deviceName: null });
      isLiveMicActive = !!active;
      updateButtonState(isLiveMicActive);
    } catch (err) {
      console.error('Failed to toggle live mic monitor:', err);
    }
  }

  function updateButtonState(active) {
    if (active) {
      liveMicBtn.classList.add('active');
    } else {
      liveMicBtn.classList.remove('active');
    }
  }

  function updateAecButtonState(active) {
    isAecActive = !!active;
    if (isAecActive) {
      aecToggleBtn.classList.add('active');
    } else {
      aecToggleBtn.classList.remove('active');
    }
  }

  async function toggleAec() {
    const nextState = !isAecActive;
    aecSyncIgnoreUntil = Date.now() + 450;
    updateAecButtonState(nextState);
    const invoker = getInvoke();
    if (!invoker) {
      return;
    }
    try {
      await invoker('set_aec', { enabled: nextState });
    } catch (err) {
      aecSyncIgnoreUntil = 0;
      updateAecButtonState(!nextState);
      console.error('Failed to toggle echo cancellation:', err);
    }
  }

  liveMicBtn.addEventListener('click', toggleLiveMic);
  aecToggleBtn.addEventListener('click', toggleAec);
  window.addEventListener('180hz:aec-sync', (ev) => {
    if (Date.now() < aecSyncIgnoreUntil) return;
    if (typeof ev.detail === 'boolean' && ev.detail !== isAecActive) {
      updateAecButtonState(ev.detail);
    }
  });

  // 4. Initial status sync
  async function initStatus() {
    const invoker = getInvoke();
    if (invoker) {
      try {
        const status = await invoker('get_live_test_status');
        if (status && status.is_active) {
          isLiveMicActive = true;
          updateButtonState(true);
        }
      } catch (_) {}
      try {
        const cfg = await invoker('get_config');
        if (cfg && typeof cfg.aec_enabled === 'boolean') {
          updateAecButtonState(cfg.aec_enabled);
        }
      } catch (_) {}
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initStatus);
  } else {
    initStatus();
  }
})();
