// 180Hz NC UI Controller
const invoke = window.__TAURI__ ? (window.__TAURI__.core ? window.__TAURI__.core.invoke : window.__TAURI__.invoke) : null;

// UI Elements
const liveTestBtn = document.getElementById('liveTestBtn');
const liveTestText = document.getElementById('liveTestText');

const micSelect = document.getElementById('micSelect');
const micVolumeSlider = document.getElementById('micVolumeSlider');
const micVolText = document.getElementById('micVolText');
const micMeterFill = document.getElementById('micMeterFill');
const micMeterDb = document.getElementById('micMeterDb');

const speakerSelect = document.getElementById('speakerSelect');
const speakerVolumeSlider = document.getElementById('speakerVolumeSlider');
const speakerVolText = document.getElementById('speakerVolText');

const autoSensitivityToggle = document.getElementById('autoSensitivityToggle');
const manualSensitivitySection = document.getElementById('manualSensitivitySection');
const sensitivitySlider = document.getElementById('sensitivitySlider');
const sensitivityText = document.getElementById('sensitivityText');

const segButtons = document.querySelectorAll('.seg-btn');

const aecToggle = document.getElementById('aecToggle');

const pttToggle = document.getElementById('pttToggle');
const pttControls = document.getElementById('pttControls');
const pttKeyBtn = document.getElementById('pttKeyBtn');
const pttStatusBadge = document.getElementById('pttStatusBadge');

let currentPttKey = 'Space';
let isBindingPttKey = false;
let isPttPressed = false;
let isLiveTestActive = false;

// ----------------------------------------------------------------------------
// Initialization
// ----------------------------------------------------------------------------

async function init() {
  if (!invoke) return;

  try {
    // 1. Load Audio Devices
    await loadDevices();

    // 2. Load Current Configuration
    const cfg = await invoke('get_config');
    applyConfig(cfg);

    // 3. Load Live Test Status
    const liveStatus = await invoke('get_live_test_status');
    updateLiveTestUI(liveStatus.is_active);
  } catch (err) {
    console.error('Initialization error:', err);
  }

  // Start polling for VU meters and status
  setInterval(pollMetrics, 60);
  setInterval(loadDevices, 4000);
}

async function loadDevices() {
  if (!invoke) return;
  try {
    const inputDevs = await invoke('get_input_devices');
    populateDropdown(micSelect, inputDevs, 'Default Microphone');

    const outputDevs = await invoke('get_output_devices');
    populateDropdown(speakerSelect, outputDevs, 'Default Speaker / Headphones');
  } catch (e) {}
}

function populateDropdown(selectEl, devices, defaultLabel) {
  const currentVal = selectEl.value;
  selectEl.innerHTML = '';

  const defOpt = document.createElement('option');
  defOpt.value = '';
  defOpt.textContent = defaultLabel;
  selectEl.appendChild(defOpt);

  if (Array.isArray(devices)) {
    devices.forEach((d) => {
      // Exclude virtual cables from monitor speaker dropdown
      if (selectEl === speakerSelect) {
        const lower = (d.name || '').toLowerCase();
        if (lower.includes('cable') || lower.includes('vb-audio')) return;
      }
      const opt = document.createElement('option');
      opt.value = d.name;
      opt.textContent = d.name + (d.is_default ? ' (System Default)' : '');
      selectEl.appendChild(opt);
    });
  }

  if (currentVal) {
    selectEl.value = currentVal;
  }
}

function applyConfig(cfg) {
  if (!cfg) return;

  // Mic & Speaker
  if (cfg.selected_mic) micSelect.value = cfg.selected_mic;
  if (cfg.selected_speaker) speakerSelect.value = cfg.selected_speaker;

  // Volumes
  const micPct = Math.round((cfg.mic_volume || 1.0) * 100);
  micVolumeSlider.value = micPct;
  micVolText.textContent = `${micPct}%`;

  const spkPct = Math.round((cfg.speaker_volume || 1.0) * 100);
  speakerVolumeSlider.value = spkPct;
  speakerVolText.textContent = `${spkPct}%`;

  // Auto Sensitivity
  const isAuto = cfg.auto_sensitivity !== false;
  autoSensitivityToggle.checked = isAuto;
  manualSensitivitySection.classList.toggle('disabled', isAuto);
  sensitivitySlider.disabled = isAuto;

  const thresh = Math.round(cfg.sensitivity_threshold !== undefined ? cfg.sensitivity_threshold : -50);
  sensitivitySlider.value = thresh;
  sensitivityText.textContent = `${thresh} dB`;

  // Noise Suppression Mode (Krisp, Standard, None)
  let activeMode = 'Krisp';
  if (cfg.bypass) {
    activeMode = 'None';
  } else if (cfg.mode === 'Standard') {
    activeMode = 'Standard';
  } else {
    activeMode = 'Krisp';
  }
  updateNoiseModeUI(activeMode);

  // AEC
  aecToggle.checked = !!cfg.aec_enabled;

  // Push to Talk
  const isPtt = !!cfg.push_to_talk;
  pttToggle.checked = isPtt;
  pttControls.classList.toggle('disabled', !isPtt);

  currentPttKey = cfg.ptt_key || 'Space';
  pttKeyBtn.textContent = currentPttKey;
  updatePttStatus(false);
}

// ----------------------------------------------------------------------------
// Live Voice Tester
// ----------------------------------------------------------------------------

function updateLiveTestUI(active) {
  isLiveTestActive = !!active;
  if (isLiveTestActive) {
    liveTestBtn.classList.add('active');
    liveTestText.textContent = 'Testing Voice';
  } else {
    liveTestBtn.classList.remove('active');
    liveTestText.textContent = 'Test Voice';
  }
}

liveTestBtn.addEventListener('click', async () => {
  if (!invoke) return;
  try {
    const spkName = speakerSelect.value || null;
    const active = await invoke('toggle_live_test', { deviceName: spkName });
    updateLiveTestUI(active);
  } catch (err) {
    console.error('Failed to toggle live test:', err);
  }
});

// ----------------------------------------------------------------------------
// Audio Controls
// ----------------------------------------------------------------------------

micSelect.addEventListener('change', () => {
  if (invoke) invoke('set_input_device', { name: micSelect.value || null });
});

micVolumeSlider.addEventListener('input', () => {
  const val = parseInt(micVolumeSlider.value, 10);
  micVolText.textContent = `${val}%`;
  if (invoke) invoke('set_mic_volume', { volume: val / 100.0 });
});

speakerSelect.addEventListener('change', () => {
  if (invoke) invoke('set_output_device', { name: speakerSelect.value || null });
});

speakerVolumeSlider.addEventListener('input', () => {
  const val = parseInt(speakerVolumeSlider.value, 10);
  speakerVolText.textContent = `${val}%`;
  if (invoke) invoke('set_speaker_volume', { volume: val / 100.0 });
});

// ----------------------------------------------------------------------------
// Sensitivity Controls
// ----------------------------------------------------------------------------

autoSensitivityToggle.addEventListener('change', () => {
  const enabled = autoSensitivityToggle.checked;
  manualSensitivitySection.classList.toggle('disabled', enabled);
  sensitivitySlider.disabled = enabled;
  if (invoke) invoke('set_auto_sensitivity', { enabled });
});

sensitivitySlider.addEventListener('input', () => {
  const val = parseFloat(sensitivitySlider.value);
  sensitivityText.textContent = `${val} dB`;
  if (invoke) invoke('set_sensitivity_threshold', { threshold: val });
});

// ----------------------------------------------------------------------------
// Noise Suppression Mode (Krisp, Standard, None)
// ----------------------------------------------------------------------------

function updateNoiseModeUI(mode) {
  segButtons.forEach((btn) => {
    if (btn.dataset.mode === mode) {
      btn.classList.add('active');
    } else {
      btn.classList.remove('active');
    }
  });
}

segButtons.forEach((btn) => {
  btn.addEventListener('click', () => {
    const mode = btn.dataset.mode;
    updateNoiseModeUI(mode);
    if (invoke) invoke('set_noise_mode', { mode });
  });
});

// ----------------------------------------------------------------------------
// Echo Cancellation
// ----------------------------------------------------------------------------

aecToggle.addEventListener('change', () => {
  if (invoke) invoke('set_aec', { enabled: aecToggle.checked });
});

// ----------------------------------------------------------------------------
// Push to Talk
// ----------------------------------------------------------------------------

function updatePttStatus(transmitting) {
  if (!pttToggle.checked) {
    pttStatusBadge.textContent = 'Muted';
    pttStatusBadge.classList.remove('transmitting');
    return;
  }
  if (transmitting) {
    pttStatusBadge.textContent = 'Transmitting';
    pttStatusBadge.classList.add('transmitting');
  } else {
    pttStatusBadge.textContent = 'Muted';
    pttStatusBadge.classList.remove('transmitting');
  }
}

pttToggle.addEventListener('change', () => {
  const enabled = pttToggle.checked;
  pttControls.classList.toggle('disabled', !enabled);
  if (invoke) invoke('set_push_to_talk', { enabled });
  updatePttStatus(false);
});

// PTT Key Binding
pttKeyBtn.addEventListener('click', () => {
  if (isBindingPttKey) return;
  isBindingPttKey = true;
  pttKeyBtn.textContent = 'Press a key...';
  pttKeyBtn.classList.add('recording');

  const onKeyDown = (e) => {
    e.preventDefault();
    e.stopPropagation();

    let keyName = e.key;
    if (e.code === 'Space') keyName = 'Space';
    else if (e.code.startsWith('Key')) keyName = e.code.replace('Key', '');
    else if (e.code === 'ControlLeft' || e.code === 'ControlRight') keyName = 'Ctrl';
    else if (e.code === 'AltLeft' || e.code === 'AltRight') keyName = 'Alt';
    else if (e.code === 'ShiftLeft' || e.code === 'ShiftRight') keyName = 'Shift';

    currentPttKey = keyName;
    pttKeyBtn.textContent = keyName;
    pttKeyBtn.classList.remove('recording');
    isBindingPttKey = false;

    if (invoke) invoke('set_ptt_key', { key: keyName });
    window.removeEventListener('keydown', onKeyDown, true);
  };

  window.addEventListener('keydown', onKeyDown, true);
});

// Push-to-Talk Keyboard Activation inside App Window
function isKeyMatchingPtt(e) {
  const normKey = currentPttKey.toLowerCase();
  if (normKey === 'space' && e.code === 'Space') return true;
  if (normKey === 'ctrl' && (e.key === 'Control' || e.code.includes('Control'))) return true;
  if (normKey === 'alt' && (e.key === 'Alt' || e.code.includes('Alt'))) return true;
  if (normKey === 'shift' && (e.key === 'Shift' || e.code.includes('Shift'))) return true;
  return e.key.toLowerCase() === normKey || e.code.toLowerCase() === `key${normKey}`;
}

window.addEventListener('keydown', (e) => {
  if (isBindingPttKey || !pttToggle.checked) return;
  if (isKeyMatchingPtt(e) && !isPttPressed) {
    isPttPressed = true;
    updatePttStatus(true);
    if (invoke) invoke('set_ptt_state', { active: true });
  }
});

window.addEventListener('keyup', (e) => {
  if (isBindingPttKey || !pttToggle.checked) return;
  if (isKeyMatchingPtt(e)) {
    isPttPressed = false;
    updatePttStatus(false);
    if (invoke) invoke('set_ptt_state', { active: false });
  }
});

// ----------------------------------------------------------------------------
// Polling for VU Meter & Live Test Status
// ----------------------------------------------------------------------------

async function pollMetrics() {
  if (!invoke) return;
  try {
    const metrics = await invoke('get_metrics');
    if (metrics) {
      const inDb = metrics.input_peak_db;
      // Map -60 dB .. 0 dB to 0% .. 100%
      const clamped = Math.max(-60, Math.min(0, inDb));
      const pct = Math.round(((clamped + 60) / 60) * 100);

      micMeterFill.style.width = `${pct}%`;
      micMeterDb.textContent = inDb > -58 ? `${Math.round(inDb)} dB` : '-60 dB';
    }

    const liveStatus = await invoke('get_live_test_status');
    if (liveStatus && liveStatus.is_active !== isLiveTestActive) {
      updateLiveTestUI(liveStatus.is_active);
    }
  } catch (e) {}
}

// Start
window.addEventListener('DOMContentLoaded', init);
