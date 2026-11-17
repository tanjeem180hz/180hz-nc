// 180Hz NC UI Controller

// Robust IPC Invoke Wrapper supporting Tauri v2 and fallback internals
function getInvoke() {
  if (window.__TAURI__) {
    if (window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
      return window.__TAURI__.core.invoke;
    }
    if (typeof window.__TAURI__.invoke === 'function') {
      return window.__TAURI__.invoke;
    }
  }
  if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
    return window.__TAURI_INTERNALS__.invoke;
  }
  return null;
}

async function invoke(cmd, args) {
  let fn = getInvoke();
  if (!fn) {
    // Wait up to 3 seconds for Tauri IPC bridge to attach
    for (let i = 0; i < 30; i++) {
      await new Promise((r) => setTimeout(r, 100));
      fn = getInvoke();
      if (fn) break;
    }
  }
  if (!fn) {
    console.error(`Tauri IPC not available for ${cmd}`);
    throw new Error(`Tauri IPC not available for ${cmd}`);
  }
  return fn(cmd, args);
}

// UI Elements - Live Voice Tester
const liveVoiceBtn = document.getElementById('liveVoiceBtn');
const liveVoiceIconOff = document.getElementById('liveVoiceIconOff');
const liveVoiceIconOn = document.getElementById('liveVoiceIconOn');
const liveVoiceTitle = document.getElementById('liveVoiceTitle');
const liveVoiceDesc = document.getElementById('liveVoiceDesc');

// UI Elements - Audio Devices & Controls
const micSelect = document.getElementById('micSelect');
const micAutoBadge = document.getElementById('micAutoBadge');
const micVolumeSlider = document.getElementById('micVolumeSlider');
const micVolText = document.getElementById('micVolText');
const micMeterFill = document.getElementById('micMeterFill');
const micMeterDb = document.getElementById('micMeterDb');

const speakerSelect = document.getElementById('speakerSelect');
const spkAutoBadge = document.getElementById('spkAutoBadge');
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
// Initialization & Automatic Device Detection
// ----------------------------------------------------------------------------

async function init() {
  console.log('180Hz NC: Initializing audio device detection...');
  try {
    // 1. Fetch Config, Microphone list, and Speaker list in parallel immediately
    const [cfg, inputDevs, outputDevs] = await Promise.all([
      invoke('get_config').catch((e) => {
        console.error('get_config error:', e);
        return null;
      }),
      invoke('get_input_devices').catch((e) => {
        console.error('get_input_devices error:', e);
        return [];
      }),
      invoke('get_output_devices').catch((e) => {
        console.error('get_output_devices error:', e);
        return [];
      }),
    ]);

    // 2. Automatically populate & select microphone & speaker
    autoSelectAndPopulateMic(inputDevs, cfg ? cfg.selected_mic : null);
    autoSelectAndPopulateSpeaker(outputDevs, cfg ? cfg.selected_speaker : null);

    // 3. Apply Remaining Config
    if (cfg) {
      applyConfig(cfg);
    }

    // 4. Load Live Test Status
    try {
      const liveStatus = await invoke('get_live_test_status');
      updateLiveTestUI(liveStatus.is_active);
    } catch (err) {}
  } catch (err) {
    console.error('Initialization error:', err);
  }

  // Start polling for VU meters and periodic device checks
  setInterval(pollMetrics, 70);
  setInterval(() => loadDevices(), 3000);
}

async function loadDevices(cachedCfg) {
  try {
    const [inputDevs, outputDevs, cfg] = await Promise.all([
      invoke('get_input_devices').catch(() => []),
      invoke('get_output_devices').catch(() => []),
      cachedCfg ? Promise.resolve(cachedCfg) : invoke('get_config').catch(() => null),
    ]);

    if (Array.isArray(inputDevs) && inputDevs.length > 0) {
      autoSelectAndPopulateMic(inputDevs, cfg ? cfg.selected_mic : null);
    }
    if (Array.isArray(outputDevs) && outputDevs.length > 0) {
      autoSelectAndPopulateSpeaker(outputDevs, cfg ? cfg.selected_speaker : null);
    }
  } catch (e) {
    console.error('Error refreshing devices:', e);
  }
}

function autoSelectAndPopulateMic(devices, savedMicName) {
  if (!Array.isArray(devices) || devices.length === 0) {
    micSelect.innerHTML = '<option value="">No microphone detected</option>';
    if (micAutoBadge) {
      micAutoBadge.textContent = 'Not Found';
      micAutoBadge.style.background = 'rgba(239, 68, 68, 0.2)';
      micAutoBadge.style.color = '#f87171';
    }
    return;
  }

  const hasSaved = savedMicName && devices.some((d) => d.name === savedMicName);
  const defaultDev = devices.find((d) => d.is_default) || devices[0];
  const targetMic = hasSaved ? savedMicName : defaultDev.name;

  micSelect.innerHTML = '';
  devices.forEach((d) => {
    const opt = document.createElement('option');
    opt.value = d.name;
    opt.textContent = d.name + (d.is_default ? ' (Default)' : '');
    if (d.name === targetMic) {
      opt.selected = true;
    }
    micSelect.appendChild(opt);
  });

  micSelect.value = targetMic;

  if (micAutoBadge) {
    micAutoBadge.textContent = 'Auto Detected';
    micAutoBadge.style.background = 'rgba(16, 185, 129, 0.15)';
    micAutoBadge.style.color = '#34d399';
    micAutoBadge.title = `Active Mic: ${targetMic}`;
  }

  // Persist newly selected microphone if none was saved
  if (!hasSaved && targetMic) {
    invoke('set_input_device', { name: targetMic }).catch(() => {});
  }
}

function autoSelectAndPopulateSpeaker(devices, savedSpkName) {
  if (!Array.isArray(devices) || devices.length === 0) {
    speakerSelect.innerHTML = '<option value="">No headphones/speakers detected</option>';
    if (spkAutoBadge) {
      spkAutoBadge.textContent = 'Not Found';
      spkAutoBadge.style.background = 'rgba(239, 68, 68, 0.2)';
      spkAutoBadge.style.color = '#f87171';
    }
    return;
  }

  // Exclude any virtual cables from speaker monitoring list
  const physicalDevs = devices.filter((d) => {
    const lower = (d.name || '').toLowerCase();
    return !lower.includes('cable') && !lower.includes('vb-audio');
  });

  const listToUse = physicalDevs.length > 0 ? physicalDevs : devices;

  const hasSaved = savedSpkName && listToUse.some((d) => d.name === savedSpkName);
  const defaultDev = listToUse.find((d) => d.is_default) || listToUse[0];
  const targetSpk = hasSaved ? savedSpkName : defaultDev.name;

  speakerSelect.innerHTML = '';
  listToUse.forEach((d) => {
    const opt = document.createElement('option');
    opt.value = d.name;
    opt.textContent = d.name + (d.is_default ? ' (Default)' : '');
    if (d.name === targetSpk) {
      opt.selected = true;
    }
    speakerSelect.appendChild(opt);
  });

  speakerSelect.value = targetSpk;

  if (spkAutoBadge) {
    spkAutoBadge.textContent = 'Auto Detected';
    spkAutoBadge.style.background = 'rgba(16, 185, 129, 0.15)';
    spkAutoBadge.style.color = '#34d399';
    spkAutoBadge.title = `Active Speaker: ${targetSpk}`;
  }

  // Persist newly selected speaker if none was saved
  if (!hasSaved && targetSpk) {
    invoke('set_output_device', { name: targetSpk }).catch(() => {});
  }
}

function applyConfig(cfg) {
  if (!cfg) return;

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
// Live Voice Tester (Middle Point Icon Button)
// ----------------------------------------------------------------------------

function updateLiveTestUI(active) {
  isLiveTestActive = !!active;
  if (isLiveTestActive) {
    liveVoiceBtn.classList.add('active');
    liveVoiceIconOff.style.display = 'none';
    liveVoiceIconOn.style.display = 'block';
    liveVoiceTitle.innerHTML = '<span class="live-pulse-dot">●</span> Live Voice: ON';
    liveVoiceDesc.textContent = 'Hearing yourself live in headphones. Click again to turn off.';
  } else {
    liveVoiceBtn.classList.remove('active');
    liveVoiceIconOff.style.display = 'block';
    liveVoiceIconOn.style.display = 'none';
    liveVoiceTitle.textContent = 'Test Your Voice Live';
    liveVoiceDesc.textContent = 'Click icon to hear your voice live in headphones';
  }
}

liveVoiceBtn.addEventListener('click', async () => {
  try {
    const spkName = speakerSelect.value || null;
    const active = await invoke('toggle_live_test', { deviceName: spkName });
    updateLiveTestUI(active);
  } catch (err) {
    console.error('Failed to toggle live voice tester:', err);
  }
});

// ----------------------------------------------------------------------------
// Audio Controls
// ----------------------------------------------------------------------------

micSelect.addEventListener('change', () => {
  invoke('set_input_device', { name: micSelect.value || null }).catch(() => {});
});

micVolumeSlider.addEventListener('input', () => {
  const val = parseInt(micVolumeSlider.value, 10);
  micVolText.textContent = `${val}%`;
  invoke('set_mic_volume', { volume: val / 100.0 }).catch(() => {});
});

speakerSelect.addEventListener('change', () => {
  invoke('set_output_device', { name: speakerSelect.value || null }).catch(() => {});
});

speakerVolumeSlider.addEventListener('input', () => {
  const val = parseInt(speakerVolumeSlider.value, 10);
  speakerVolText.textContent = `${val}%`;
  invoke('set_speaker_volume', { volume: val / 100.0 }).catch(() => {});
});

// ----------------------------------------------------------------------------
// Sensitivity Controls
// ----------------------------------------------------------------------------

autoSensitivityToggle.addEventListener('change', () => {
  const enabled = autoSensitivityToggle.checked;
  manualSensitivitySection.classList.toggle('disabled', enabled);
  sensitivitySlider.disabled = enabled;
  invoke('set_auto_sensitivity', { enabled }).catch(() => {});
});

sensitivitySlider.addEventListener('input', () => {
  const val = parseFloat(sensitivitySlider.value);
  sensitivityText.textContent = `${val} dB`;
  invoke('set_sensitivity_threshold', { threshold: val }).catch(() => {});
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
    invoke('set_noise_mode', { mode }).catch(() => {});
  });
});

// ----------------------------------------------------------------------------
// Echo Cancellation
// ----------------------------------------------------------------------------

aecToggle.addEventListener('change', () => {
  invoke('set_aec', { enabled: aecToggle.checked }).catch(() => {});
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
  invoke('set_push_to_talk', { enabled }).catch(() => {});
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

    invoke('set_ptt_key', { key: keyName }).catch(() => {});
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
    invoke('set_ptt_state', { active: true }).catch(() => {});
  }
});

window.addEventListener('keyup', (e) => {
  if (isBindingPttKey || !pttToggle.checked) return;
  if (isKeyMatchingPtt(e)) {
    isPttPressed = false;
    updatePttStatus(false);
    invoke('set_ptt_state', { active: false }).catch(() => {});
  }
});

// ----------------------------------------------------------------------------
// Polling for VU Meter & Live Test Status
// ----------------------------------------------------------------------------

async function pollMetrics() {
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

// Guarantee execution whether DOM is already loaded or in progress
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
