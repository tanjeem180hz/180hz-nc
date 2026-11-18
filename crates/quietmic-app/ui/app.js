// 180Hz NC - Signal Lab Controller
// Modern High-End Studio Hardware & Real-Time Telemetry

// Immediately apply synchronous URL/hash overrides before any rendering or async IPC
(function checkImmediateUrlOverrides() {
  try {
    const rawQuery = window.location.search || (window.location.hash ? window.location.hash.substring(1) : '');
    const urlParams = new URLSearchParams(rawQuery);
    const themeParam = urlParams.get('theme');
    if (themeParam) {
      document.documentElement.dataset.theme = themeParam;
    }
    const tabParam = urlParams.get('tab');
    if (tabParam) {
      window.__initialTab = tabParam;
    }
  } catch (_) {}
})();

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
    // Wait up to 1 second for Tauri IPC bridge to attach
    for (let i = 0; i < 10; i++) {
      await new Promise((r) => setTimeout(r, 100));
      fn = getInvoke();
      if (fn) break;
    }
  }
  if (!fn) {
    return getMockResponse(cmd, args);
  }
  return fn(cmd, args);
}

function getMockResponse(cmd, _args) {
  switch (cmd) {
    case 'get_config':
      return Promise.resolve({
        theme: document.documentElement.dataset.theme || 'night-lab',
        mic_volume: 1.0,
        speaker_volume: 1.0,
        strength: 1.0,
        mode: 'MaxQuality',
        bypass: false,
        auto_sensitivity: true,
        sensitivity_threshold: -50.0,
        selected_mic: 'Microphone (Focusrite Scarlett 2i2)',
        selected_speaker: 'Headphones (Realtek Audio)',
        aec_enabled: false,
        push_to_talk: false,
        ptt_key: 'Space',
      });
    case 'get_input_devices':
      return Promise.resolve([
        { name: 'Microphone (Focusrite Scarlett 2i2)', is_default: true },
        { name: 'Microphone (Realtek High Definition Audio)', is_default: false },
      ]);
    case 'get_output_devices':
      return Promise.resolve([
        { name: 'Headphones (Realtek Audio)', is_default: true },
        { name: 'Speakers (Realtek Audio)', is_default: false },
      ]);
    case 'get_virtual_status':
      return Promise.resolve({ is_installed: true });
    case 'get_live_test_status':
      return Promise.resolve({ is_active: false, volume: 1.0, ab_mode: 'Cleaned' });
    case 'get_metrics': {
      const raw = [];
      const clean = [];
      for (let i = 0; i < 32; i++) {
        const ph = (i / 32) * Math.PI * 4;
        const speech = Math.sin(ph) * 0.45;
        const noise = (Math.random() - 0.5) * 0.35;
        raw.push(speech + noise);
        clean.push(speech * 0.85);
      }
      return Promise.resolve({
        frame_time_us: 5,
        input_peak_db: -18.4,
        output_peak_db: -21.2,
        underruns: 0,
        overruns: 0,
        frames_received: 198400,
        frames_processed: 198400,
        frames_passthrough: 0,
        fallback_active: false,
        noise_reduction_db: 38.6,
        suppression_active: true,
        aec_enabled: false,
        aec_ref_device: 'Headphones (Realtek Audio)',
        aec_ref_sample_rate: 48000,
        aec_ref_peak_db: -90.0,
        aec_frames_processed: 0,
        aec_erle_db: 42.5,
        aec_delay_ms: 18.2,
        aec_endpoint: 'Headphones',
        loop_warning: false,
        raw_waveform: raw,
        clean_waveform: clean,
      });
    }
    case 'set_theme':
    case 'set_bypass':
    case 'set_strength':
    case 'set_noise_mode':
    case 'set_mic_volume':
    case 'set_speaker_volume':
    case 'set_auto_sensitivity':
    case 'set_sensitivity_threshold':
    case 'set_push_to_talk':
    case 'set_ptt_key':
    case 'set_ptt_state':
    case 'set_aec':
    case 'set_input_device':
    case 'set_output_device':
    case 'set_live_test_ab':
      return Promise.resolve();
    case 'toggle_live_test':
      return Promise.resolve(true);
    case 'run_echo_doctor_command':
      return Promise.resolve({
        loop_severity: 'CLEAN',
        plain_explanation: 'All Windows playback endpoints and communications routes are correctly configured. No feedback loop detected.',
        step_by_step_fix: [],
        discord_screen_share_tips: [
          'Share an Application Window instead of Entire Screen.',
          'Set Discord Input to CABLE Output and Output to your Headphones.'
        ]
      });
    default:
      return Promise.resolve({});
  }
}

// ----------------------------------------------------------------------------
// UI Elements & State Cache
// ----------------------------------------------------------------------------

// Header & Status
const themeButtons = document.querySelectorAll('.theme-btn');
const engineStatusPill = document.getElementById('engineStatusPill');
const engineStatusText = document.getElementById('engineStatusText');

// Tabs
const tabButtons = document.querySelectorAll('.tab-btn');
const tabPanels = document.querySelectorAll('.tab-content');

// Hero Module (Tab 1: LAB)
const masterPowerBtn = document.getElementById('masterPowerBtn');
const masterPowerLabel = document.getElementById('masterPowerLabel');
const orbNoiseStatus = document.getElementById('orbNoiseStatus');
const orbVoiceStatus = document.getElementById('orbVoiceStatus');
const noiseReductionBadge = document.getElementById('noiseReductionBadge');
const hwModeButtons = document.querySelectorAll('.hw-mode-btn');
const strengthSlider = document.getElementById('strengthSlider');
const strengthValText = document.getElementById('strengthValText');
const framesProcessedText = document.getElementById('framesProcessedText');
const framesPassthroughText = document.getElementById('framesPassthroughText');
const dspLatencyText = document.getElementById('dspLatencyText');

// Audio Routing & Meters (Tab 2: DEVICES)
const micSelect = document.getElementById('micSelect');
const micAutoBadge = document.getElementById('micAutoBadge');
const micVolumeSlider = document.getElementById('micVolumeSlider');
const micVolText = document.getElementById('micVolText');
const micMeterFill = document.getElementById('micMeterFill');
const micMeterPeak = document.getElementById('micMeterPeak');
const micMeterDb = document.getElementById('micMeterDb');
const clippingIndicator = document.getElementById('clippingIndicator');

const speakerSelect = document.getElementById('speakerSelect');
const spkAutoBadge = document.getElementById('spkAutoBadge');
const speakerVolumeSlider = document.getElementById('speakerVolumeSlider');
const speakerVolText = document.getElementById('speakerVolText');
const virtualCableBadge = document.getElementById('virtualCableBadge');

// Studio Controls (Tab 3: STUDIO)
const liveVoiceBtn = document.getElementById('liveVoiceBtn');
const liveVoiceIconOff = document.getElementById('liveVoiceIconOff');
const liveVoiceIconOn = document.getElementById('liveVoiceIconOn');
const liveVoiceTitle = document.getElementById('liveVoiceTitle');
const liveVoiceDesc = document.getElementById('liveVoiceDesc');
const abOriginalBtn = document.getElementById('abOriginalBtn');
const abCleanedBtn = document.getElementById('abCleanedBtn');

const recordBufferBtn = document.getElementById('recordBufferBtn');
const playReplayBtn = document.getElementById('playReplayBtn');
const exportWavBtn = document.getElementById('exportWavBtn');
const recordStatusBadge = document.getElementById('recordStatusBadge');

const autoSensitivityToggle = document.getElementById('autoSensitivityToggle');
const manualSensitivitySection = document.getElementById('manualSensitivitySection');
const sensitivitySlider = document.getElementById('sensitivitySlider');
const sensitivityText = document.getElementById('sensitivityText');

const pttToggle = document.getElementById('pttToggle');
const pttControls = document.getElementById('pttControls');
const pttKeyBtn = document.getElementById('pttKeyBtn');
const pttStatusBadge = document.getElementById('pttStatusBadge');

// Diagnostics (Tab 4: DIAGNOSTICS)
const aecToggle = document.getElementById('aecToggle');
const aecDetails = document.getElementById('aecDetails');
const aecEndpointBadge = document.getElementById('aecEndpointBadge');
const loopWarningPill = document.getElementById('loopWarningPill');
const aecRefDeviceText = document.getElementById('aecRefDeviceText');
const aecRefLevelText = document.getElementById('aecRefLevelText');
const aecErleText = document.getElementById('aecErleText');
const aecDelayText = document.getElementById('aecDelayText');
const aecFramesText = document.getElementById('aecFramesText');
const echoDoctorBtn = document.getElementById('echoDoctorBtn');
const aecTestBtn = document.getElementById('aecTestBtn');
const pipelineSelfTestBtn = document.getElementById('pipelineSelfTestBtn');
const pipelineDiagBtn = document.getElementById('pipelineDiagBtn');

// Modals
const echoDoctorModal = document.getElementById('echoDoctorModal');
const closeEchoDoctorBtn = document.getElementById('closeEchoDoctorBtn');
const dismissEchoDoctorBtn = document.getElementById('dismissEchoDoctorBtn');
const doctorSeverityBanner = document.getElementById('doctorSeverityBanner');
const doctorSeverityIcon = document.getElementById('doctorSeverityIcon');
const doctorSeverityText = document.getElementById('doctorSeverityText');
const doctorExplanation = document.getElementById('doctorExplanation');
const doctorFixSection = document.getElementById('doctorFixSection');
const doctorFixList = document.getElementById('doctorFixList');
const doctorDiscordTips = document.getElementById('doctorDiscordTips');

const diagnosticResultModal = document.getElementById('diagnosticResultModal');
const diagModalTitle = document.getElementById('diagModalTitle');
const diagModalBody = document.getElementById('diagModalBody');
const closeDiagModalBtn = document.getElementById('closeDiagModalBtn');
const dismissDiagModalBtn = document.getElementById('dismissDiagModalBtn');

// Runtime State
let isBypassed = false;
let currentMode = 'MaxQuality';
let currentTheme = 'night-lab';
let currentPttKey = 'Space';
let isBindingPttKey = false;
let isPttPressed = false;
let isLiveTestActive = false;
let abMode = 'Cleaned';
let peakHoldDb = -60.0;
let peakHoldDecayTimer = 0;
let metricsPollTimer = null;
let devicePollTimer = null;

// ----------------------------------------------------------------------------
// 1. Theme Management
// ----------------------------------------------------------------------------

function applyTheme(themeName) {
  currentTheme = themeName || 'night-lab';
  document.documentElement.dataset.theme = currentTheme;

  themeButtons.forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.theme === currentTheme);
  });

  if (window.SignalOrb && typeof window.SignalOrb.triggerRedraw === 'function') {
    window.SignalOrb.triggerRedraw();
  }
}

themeButtons.forEach((btn) => {
  btn.addEventListener('click', () => {
    const theme = btn.dataset.theme;
    applyTheme(theme);
    invoke('set_theme', { theme }).catch(() => {});
  });
});

// ----------------------------------------------------------------------------
// 2. Navigation Tabs
// ----------------------------------------------------------------------------

tabButtons.forEach((btn) => {
  btn.addEventListener('click', () => {
    const targetTab = btn.dataset.tab;

    tabButtons.forEach((b) => {
      const isSelected = b === btn;
      b.classList.toggle('active', isSelected);
      b.setAttribute('aria-selected', isSelected ? 'true' : 'false');
    });

    tabPanels.forEach((p) => {
      p.classList.toggle('active', p.id === targetTab);
    });
  });
});

// ----------------------------------------------------------------------------
// 3. Hero Module Controls (Master Power, Modes, Strength)
// ----------------------------------------------------------------------------

function updateMasterPowerUI(bypassed, fallbackActive) {
  isBypassed = !!bypassed;
  if (fallbackActive) {
    masterPowerBtn.className = 'master-power-btn bypassed';
    masterPowerLabel.textContent = 'ERROR';
  } else if (isBypassed) {
    masterPowerBtn.className = 'master-power-btn bypassed';
    masterPowerLabel.textContent = 'BYPASS';
  } else {
    masterPowerBtn.className = 'master-power-btn active';
    masterPowerLabel.textContent = 'ACTIVE';
  }
}

masterPowerBtn.addEventListener('click', async () => {
  const newBypass = !isBypassed;
  updateMasterPowerUI(newBypass, false);
  try {
    await invoke('set_bypass', { bypass: newBypass });
  } catch (err) {
    console.error('Failed to toggle bypass:', err);
  }
});

function updateHwModeUI(mode) {
  currentMode = mode;
  hwModeButtons.forEach((btn) => {
    const isAct = btn.dataset.mode === mode;
    btn.classList.toggle('active', isAct);
    btn.setAttribute('aria-checked', isAct ? 'true' : 'false');
  });

  if (mode === 'None') {
    updateMasterPowerUI(true, false);
  }
}

hwModeButtons.forEach((btn) => {
  btn.addEventListener('click', () => {
    const mode = btn.dataset.mode;
    updateHwModeUI(mode);
    invoke('set_noise_mode', { mode }).catch(() => {});
  });
});

strengthSlider.addEventListener('input', () => {
  const val = parseInt(strengthSlider.value, 10);
  strengthValText.textContent = `${val}%`;
  invoke('set_strength', { strength: val / 100.0 }).catch(() => {});
});

// ----------------------------------------------------------------------------
// 4. Device Routing & VU Meters
// ----------------------------------------------------------------------------

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
      micAutoBadge.style.background = 'var(--accent-red-dim)';
      micAutoBadge.style.color = 'var(--accent-red)';
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
    if (d.name === targetMic) opt.selected = true;
    micSelect.appendChild(opt);
  });

  micSelect.value = targetMic;

  if (micAutoBadge) {
    micAutoBadge.textContent = 'Auto Detected';
    micAutoBadge.style.background = 'var(--accent-green-dim)';
    micAutoBadge.style.color = 'var(--accent-green)';
    micAutoBadge.title = `Active Mic: ${targetMic}`;
  }

  if (!hasSaved && targetMic) {
    invoke('set_input_device', { name: targetMic }).catch(() => {});
  }
}

function autoSelectAndPopulateSpeaker(devices, savedSpkName) {
  if (!Array.isArray(devices) || devices.length === 0) {
    speakerSelect.innerHTML = '<option value="">No headphones/speakers detected</option>';
    if (spkAutoBadge) {
      spkAutoBadge.textContent = 'Not Found';
      spkAutoBadge.style.background = 'var(--accent-red-dim)';
      spkAutoBadge.style.color = 'var(--accent-red)';
    }
    return;
  }

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
    if (d.name === targetSpk) opt.selected = true;
    speakerSelect.appendChild(opt);
  });

  speakerSelect.value = targetSpk;

  if (spkAutoBadge) {
    spkAutoBadge.textContent = 'Auto Detected';
    spkAutoBadge.style.background = 'var(--accent-green-dim)';
    spkAutoBadge.style.color = 'var(--accent-green)';
    spkAutoBadge.title = `Active Speaker: ${targetSpk}`;
  }

  if (!hasSaved && targetSpk) {
    invoke('set_output_device', { name: targetSpk }).catch(() => {});
  }
}

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
// 5. Studio Tools (Live Voice Monitor, A/B Switch, Recorder, Sensitivity, PTT)
// ----------------------------------------------------------------------------

function updateLiveTestUI(active) {
  isLiveTestActive = !!active;
  if (isLiveTestActive) {
    liveVoiceBtn.classList.add('active');
    liveVoiceIconOff.style.display = 'none';
    liveVoiceIconOn.style.display = 'block';
    liveVoiceTitle.innerHTML = '<span style="color: var(--accent-green);">●</span> Live Monitor: ON';
    liveVoiceDesc.textContent = 'Streaming live monitor audio to headphones.';
  } else {
    liveVoiceBtn.classList.remove('active');
    liveVoiceIconOff.style.display = 'block';
    liveVoiceIconOn.style.display = 'none';
    liveVoiceTitle.textContent = 'Test Your Voice Live';
    liveVoiceDesc.textContent = 'Click to stream live monitor audio to your headphones.';
  }
}

liveVoiceBtn.addEventListener('click', async () => {
  try {
    const spkName = speakerSelect.value || null;
    const active = await invoke('toggle_live_test', { deviceName: spkName });
    updateLiveTestUI(active);
  } catch (err) {
    console.error('Failed to toggle live monitor:', err);
  }
});

abOriginalBtn.addEventListener('click', () => {
  abMode = 'Original';
  abOriginalBtn.classList.add('primary');
  abCleanedBtn.classList.remove('primary');
  invoke('set_live_test_ab', { mode: 'Original' }).catch(() => {});
});

abCleanedBtn.addEventListener('click', () => {
  abMode = 'Cleaned';
  abCleanedBtn.classList.add('primary');
  abOriginalBtn.classList.remove('primary');
  invoke('set_live_test_ab', { mode: 'Cleaned' }).catch(() => {});
});

recordBufferBtn.addEventListener('click', async () => {
  recordBufferBtn.disabled = true;
  recordBufferBtn.textContent = 'Recording (10s)...';
  recordStatusBadge.textContent = 'Recording';
  recordStatusBadge.style.color = 'var(--accent-red)';
  try {
    await invoke('start_record_and_replay');
    setTimeout(() => {
      recordBufferBtn.disabled = false;
      recordBufferBtn.textContent = '⏺ Record 10s';
      playReplayBtn.disabled = false;
      exportWavBtn.disabled = false;
      recordStatusBadge.textContent = 'Recorded Ready';
      recordStatusBadge.style.color = 'var(--accent-green)';
    }, 10000);
  } catch (e) {
    recordBufferBtn.disabled = false;
    recordBufferBtn.textContent = '⏺ Record 10s';
    recordStatusBadge.textContent = 'Error';
    alert(`Recording failed: ${e}`);
  }
});

playReplayBtn.addEventListener('click', async () => {
  playReplayBtn.disabled = true;
  playReplayBtn.textContent = 'Playing...';
  try {
    await invoke('start_replay_playback');
    setTimeout(() => {
      playReplayBtn.disabled = false;
      playReplayBtn.textContent = '▶ Playback';
    }, 10000);
  } catch (e) {
    playReplayBtn.disabled = false;
    playReplayBtn.textContent = '▶ Playback';
    alert(`Playback failed: ${e}`);
  }
});

exportWavBtn.addEventListener('click', async () => {
  exportWavBtn.disabled = true;
  try {
    const [raw, clean] = await invoke('export_recorded_wav', { dir: null });
    showDiagnosticModal('Recorded Audio Exported', `Raw input WAV:\n${raw}\n\nCleaned output WAV:\n${clean}`);
  } catch (e) {
    alert(`Export failed: ${e}`);
  } finally {
    exportWavBtn.disabled = false;
  }
});

// Sensitivity
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

// Push-to-Talk
function updatePttStatus(transmitting) {
  if (!pttToggle.checked) {
    pttStatusBadge.textContent = 'Muted';
    pttStatusBadge.style.color = 'var(--text-muted)';
    return;
  }
  if (transmitting) {
    pttStatusBadge.textContent = 'Transmitting';
    pttStatusBadge.style.color = 'var(--accent-green)';
  } else {
    pttStatusBadge.textContent = 'Muted';
    pttStatusBadge.style.color = 'var(--accent-amber)';
  }
}

pttToggle.addEventListener('change', () => {
  const enabled = pttToggle.checked;
  pttControls.classList.toggle('disabled', !enabled);
  invoke('set_push_to_talk', { enabled }).catch(() => {});
  updatePttStatus(false);
});

pttKeyBtn.addEventListener('click', () => {
  if (isBindingPttKey) return;
  isBindingPttKey = true;
  pttKeyBtn.textContent = 'Press key...';

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
    isBindingPttKey = false;

    invoke('set_ptt_key', { key: keyName }).catch(() => {});
    window.removeEventListener('keydown', onKeyDown, true);
  };

  window.addEventListener('keydown', onKeyDown, true);
});

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
// 6. Echo Cancellation & Diagnostics
// ----------------------------------------------------------------------------

aecToggle.addEventListener('change', () => {
  const en = aecToggle.checked;
  if (aecDetails) aecDetails.style.display = en ? 'flex' : 'none';
  invoke('set_aec', { enabled: en }).catch(() => {});
});

echoDoctorBtn.addEventListener('click', async () => {
  echoDoctorBtn.disabled = true;
  echoDoctorBtn.textContent = 'Diagnosing...';
  try {
    const report = await invoke('run_echo_doctor_command');
    if (report && echoDoctorModal) {
      const sev = report.loop_severity || 'CLEAN';
      if (sev === 'CRITICAL') {
        doctorSeverityBanner.style.background = 'var(--accent-red-dim)';
        doctorSeverityBanner.style.border = '1px solid var(--accent-red)';
        doctorSeverityBanner.style.color = 'var(--accent-red)';
        doctorSeverityIcon.textContent = '🚨';
        doctorSeverityText.textContent = 'Critical Loop Detected!';
      } else if (sev === 'WARNING') {
        doctorSeverityBanner.style.background = 'var(--accent-amber-dim)';
        doctorSeverityBanner.style.border = '1px solid var(--accent-amber)';
        doctorSeverityBanner.style.color = 'var(--accent-amber)';
        doctorSeverityIcon.textContent = '⚠️';
        doctorSeverityText.textContent = 'Potential Routing / Echo Warning';
      } else {
        doctorSeverityBanner.style.background = 'var(--accent-green-dim)';
        doctorSeverityBanner.style.border = '1px solid var(--accent-green)';
        doctorSeverityBanner.style.color = 'var(--accent-green)';
        doctorSeverityIcon.textContent = '✅';
        doctorSeverityText.textContent = 'Clean Audio Routing - No Loop Detected';
      }

      doctorExplanation.textContent = report.plain_explanation || 'No routing issues detected.';

      if (report.step_by_step_fix && report.step_by_step_fix.length > 0) {
        doctorFixList.innerHTML = '';
        report.step_by_step_fix.forEach((step) => {
          const li = document.createElement('li');
          li.textContent = step;
          doctorFixList.appendChild(li);
        });
        doctorFixSection.style.display = 'block';
      } else {
        doctorFixSection.style.display = 'none';
      }

      if (report.discord_screen_share_tips && report.discord_screen_share_tips.length > 0) {
        doctorDiscordTips.innerHTML = '';
        report.discord_screen_share_tips.forEach((tip) => {
          const li = document.createElement('li');
          li.textContent = tip;
          doctorDiscordTips.appendChild(li);
        });
      }

      echoDoctorModal.style.display = 'flex';
    }
  } catch (e) {
    alert(`Echo Doctor failed: ${e}`);
  } finally {
    echoDoctorBtn.disabled = false;
    echoDoctorBtn.textContent = '🩺 Echo Doctor';
  }
});

closeEchoDoctorBtn.addEventListener('click', () => {
  echoDoctorModal.style.display = 'none';
});
dismissEchoDoctorBtn.addEventListener('click', () => {
  echoDoctorModal.style.display = 'none';
});

function showDiagnosticModal(title, content) {
  diagModalTitle.textContent = title;
  diagModalBody.textContent = content;
  diagnosticResultModal.style.display = 'flex';
}

closeDiagModalBtn.addEventListener('click', () => {
  diagnosticResultModal.style.display = 'none';
});
dismissDiagModalBtn.addEventListener('click', () => {
  diagnosticResultModal.style.display = 'none';
});

aecTestBtn.addEventListener('click', async () => {
  aecTestBtn.disabled = true;
  aecTestBtn.textContent = 'Testing...';
  try {
    const res = await invoke('run_aec_speaker_test_command');
    showDiagnosticModal('AEC Speaker Test Result', res.summary || JSON.stringify(res, null, 2));
  } catch (e) {
    alert(`AEC Test failed: ${e}`);
  } finally {
    aecTestBtn.disabled = false;
    aecTestBtn.textContent = '🔊 Speaker Test';
  }
});

pipelineSelfTestBtn.addEventListener('click', async () => {
  pipelineSelfTestBtn.disabled = true;
  pipelineSelfTestBtn.textContent = 'Testing...';
  try {
    const res = await invoke('run_pipeline_self_test_command');
    showDiagnosticModal('Pipeline Self-Test Result', res.summary || JSON.stringify(res, null, 2));
  } catch (e) {
    alert(`Self-Test failed: ${e}`);
  } finally {
    pipelineSelfTestBtn.disabled = false;
    pipelineSelfTestBtn.textContent = '⚙️ Self-Test';
  }
});

pipelineDiagBtn.addEventListener('click', async () => {
  pipelineDiagBtn.disabled = true;
  pipelineDiagBtn.textContent = 'Running...';
  try {
    const res = await invoke('run_pipeline_diagnostics', { outputDir: null });
    let summary = `Pipeline Diagnostics Generated:\nProcessed ${res.length} stages:\n\n`;
    res.forEach((s) => {
      summary += `• Stage: ${s.stage_name} | Peak: ${s.peak_db.toFixed(1)} dB | RMS: ${s.rms_db.toFixed(1)} dB\n`;
    });
    showDiagnosticModal('Pipeline Stage Diagnostics (WAV Dumper)', summary);
  } catch (e) {
    alert(`Diagnostics failed: ${e}`);
  } finally {
    pipelineDiagBtn.disabled = false;
    pipelineDiagBtn.textContent = '📊 Pipeline WAVs';
  }
});

// ----------------------------------------------------------------------------
// 7. Real-Time Telemetry & Studio VU Meter Loop (0% CPU when hidden)
// ----------------------------------------------------------------------------

async function pollMetrics() {
  try {
    const metrics = await invoke('get_metrics');
    if (!metrics) return;

    // 1. Update Signal Orb Circular Oscilloscope
    if (window.SignalOrb && typeof window.SignalOrb.update === 'function') {
      window.SignalOrb.update(
        metrics.raw_waveform,
        metrics.clean_waveform,
        metrics.suppression_active,
        isBypassed
      );
    }

    // 2. Input VU Meter & Peak Hold
    const inDb = metrics.input_peak_db;
    const clamped = Math.max(-60, Math.min(0, inDb));
    const pct = Math.round(((clamped + 60) / 60) * 100);

    if (micMeterFill) micMeterFill.style.width = `${pct}%`;
    if (micMeterDb) micMeterDb.textContent = inDb > -58 ? `${Math.round(inDb)} dB` : '-60 dB';

    if (inDb > peakHoldDb) {
      peakHoldDb = inDb;
      peakHoldDecayTimer = 15;
    } else if (peakHoldDecayTimer > 0) {
      peakHoldDecayTimer--;
    } else {
      peakHoldDb = Math.max(-60, peakHoldDb - 2.5);
    }

    if (micMeterPeak) {
      const peakClamped = Math.max(-60, Math.min(0, peakHoldDb));
      const peakPct = Math.round(((peakClamped + 60) / 60) * 100);
      micMeterPeak.style.left = `${peakPct}%`;
    }

    if (clippingIndicator) {
      clippingIndicator.style.display = inDb >= -1.0 ? 'inline-block' : 'none';
    }

    // 3. Noise Reduction Badge & Orb Micro-copy
    const nrDb = Math.round(metrics.noise_reduction_db || 0);
    if (!metrics.suppression_active) {
      noiseReductionBadge.textContent = 'Bypassed (0 dB)';
      noiseReductionBadge.style.background = 'var(--accent-amber-dim)';
      noiseReductionBadge.style.color = 'var(--accent-amber)';
      orbNoiseStatus.textContent = 'Noise: pass-through';
      orbVoiceStatus.textContent = 'Voice: uncleaned';
    } else if (metrics.fallback_active) {
      noiseReductionBadge.textContent = 'Fallback Error';
      noiseReductionBadge.style.background = 'var(--accent-red-dim)';
      noiseReductionBadge.style.color = 'var(--accent-red)';
      orbNoiseStatus.textContent = 'Noise: fallback';
      orbVoiceStatus.textContent = 'Voice: raw';
    } else {
      noiseReductionBadge.textContent = nrDb > 2 ? `Active (-${nrDb} dB)` : 'Active';
      noiseReductionBadge.style.background = 'var(--accent-green-dim)';
      noiseReductionBadge.style.color = 'var(--accent-green)';
      orbNoiseStatus.textContent = nrDb > 2 ? `Noise: -${nrDb} dB` : 'Noise: pulled flat';
      orbVoiceStatus.textContent = 'Voice: pristine';
    }

    // 4. Master Power Button & Header Status Pill
    updateMasterPowerUI(!metrics.suppression_active, metrics.fallback_active);

    if (engineStatusPill && engineStatusText) {
      if (metrics.fallback_active) {
        engineStatusPill.className = 'status-pill fallback';
        engineStatusText.textContent = 'Fallback Pass-Through';
      } else if (metrics.suppression_active) {
        engineStatusPill.className = 'status-pill active';
        engineStatusText.textContent = nrDb > 2 ? `Krisp AI (-${nrDb} dB)` : 'Krisp AI Active';
      } else {
        engineStatusPill.className = 'status-pill bypassed';
        engineStatusText.textContent = 'Suppression Bypassed';
      }
    }

    // 5. Engine Micro-Telemetry
    if (framesProcessedText) framesProcessedText.textContent = metrics.frames_processed || 0;
    if (framesPassthroughText) {
      framesPassthroughText.textContent = metrics.frames_passthrough || 0;
      framesPassthroughText.style.color = metrics.frames_passthrough > 0 ? 'var(--accent-red)' : 'var(--text-main)';
    }
    if (dspLatencyText) dspLatencyText.textContent = `${metrics.frame_time_us || 0} µs`;

    // 6. AEC Telemetry
    if (aecDetails && metrics.aec_enabled) {
      aecDetails.style.display = 'flex';
      if (aecRefDeviceText) aecRefDeviceText.textContent = metrics.aec_ref_device || 'Default Output';
      if (aecRefLevelText) {
        aecRefLevelText.textContent = metrics.aec_ref_peak_db > -58 ? `${Math.round(metrics.aec_ref_peak_db)} dB` : '-100 dB';
      }
      if (aecErleText) aecErleText.textContent = `${Math.round(metrics.aec_erle_db || 0)} dB`;
      if (aecDelayText) aecDelayText.textContent = `${(metrics.aec_delay_ms || 0).toFixed(1)} ms`;
      if (aecFramesText) aecFramesText.textContent = metrics.aec_frames_processed || 0;

      if (aecEndpointBadge) {
        const ep = metrics.aec_endpoint || 'Speakers';
        aecEndpointBadge.style.display = 'inline-block';
        if (ep === 'Headphones' || ep === 'Headset') {
          aecEndpointBadge.textContent = `${ep}: Auto-Bypassed`;
          aecEndpointBadge.style.background = 'var(--accent-cyan-dim)';
          aecEndpointBadge.style.color = 'var(--accent-cyan)';
        } else {
          aecEndpointBadge.textContent = 'Speakers: AEC Active';
          aecEndpointBadge.style.background = 'var(--accent-green-dim)';
          aecEndpointBadge.style.color = 'var(--accent-green)';
        }
      }
    } else if (aecDetails) {
      aecDetails.style.display = 'none';
      if (aecEndpointBadge) aecEndpointBadge.style.display = 'none';
    }

    if (loopWarningPill) {
      loopWarningPill.style.display = metrics.loop_warning ? 'inline-block' : 'none';
    }

    // 7. Live Voice Tester Sync
    const liveStatus = await invoke('get_live_test_status').catch(() => null);
    if (liveStatus && liveStatus.is_active !== isLiveTestActive) {
      updateLiveTestUI(liveStatus.is_active);
    }
  } catch (e) {}
}

function startPolling() {
  if (!metricsPollTimer) {
    metricsPollTimer = setInterval(pollMetrics, 70);
  }
  if (!devicePollTimer) {
    devicePollTimer = setInterval(() => loadDevices(), 3000);
  }
}

function stopPolling() {
  if (metricsPollTimer) {
    clearInterval(metricsPollTimer);
    metricsPollTimer = null;
  }
  if (devicePollTimer) {
    clearInterval(devicePollTimer);
    devicePollTimer = null;
  }
}

// Listen to window visibility changes for 0% CPU consumption
document.addEventListener('visibilitychange', () => {
  if (document.hidden) {
    stopPolling();
  } else {
    pollMetrics();
    startPolling();
  }
});

// ----------------------------------------------------------------------------
// 8. Application Initialization
// ----------------------------------------------------------------------------

async function init() {
  console.log('180Hz NC - Initializing 180 Hz Signal Lab UI...');
  try {
    const [cfg, inputDevs, outputDevs] = await Promise.all([
      invoke('get_config').catch((e) => {
        console.error('get_config error:', e);
        return null;
      }),
      invoke('get_input_devices').catch(() => []),
      invoke('get_output_devices').catch(() => []),
    ]);

    if (cfg) {
      // 1. Theme & Tab overrides
      const rawQuery = window.location.search || (window.location.hash ? window.location.hash.substring(1) : '');
      const urlParams = new URLSearchParams(rawQuery);
      const urlTheme = urlParams.get('theme');
      applyTheme(urlTheme || cfg.theme || 'night-lab');

      const urlTab = urlParams.get('tab') || window.__initialTab;
      if (urlTab) {
        tabButtons.forEach((b) => {
          const isSelected = b.dataset.tab === urlTab;
          b.classList.toggle('active', isSelected);
          b.setAttribute('aria-selected', isSelected ? 'true' : 'false');
        });
        tabPanels.forEach((p) => {
          p.classList.toggle('active', p.id === urlTab);
        });
      }

      // 2. Volumes
      const micPct = Math.round((cfg.mic_volume || 1.0) * 100);
      micVolumeSlider.value = micPct;
      micVolText.textContent = `${micPct}%`;

      const spkPct = Math.round((cfg.speaker_volume || 1.0) * 100);
      speakerVolumeSlider.value = spkPct;
      speakerVolText.textContent = `${spkPct}%`;

      // 3. Suppression Strength
      const strPct = Math.round((cfg.strength !== undefined ? cfg.strength : 1.0) * 100);
      strengthSlider.value = strPct;
      strengthValText.textContent = `${strPct}%`;

      // 4. Mode
      let activeMode = 'MaxQuality';
      if (cfg.bypass || cfg.mode === 'None') activeMode = 'None';
      else if (cfg.mode === 'Light') activeMode = 'Light';
      else if (cfg.mode === 'Standard') activeMode = 'Standard';
      else activeMode = 'MaxQuality';
      updateHwModeUI(activeMode);
      updateMasterPowerUI(cfg.bypass || cfg.mode === 'None', false);

      // 5. Sensitivity
      const isAuto = cfg.auto_sensitivity !== false;
      autoSensitivityToggle.checked = isAuto;
      manualSensitivitySection.classList.toggle('disabled', isAuto);
      sensitivitySlider.disabled = isAuto;

      const thresh = Math.round(cfg.sensitivity_threshold !== undefined ? cfg.sensitivity_threshold : -50);
      sensitivitySlider.value = thresh;
      sensitivityText.textContent = `${thresh} dB`;

      // 6. AEC & PTT
      aecToggle.checked = !!cfg.aec_enabled;
      const isPtt = !!cfg.push_to_talk;
      pttToggle.checked = isPtt;
      pttControls.classList.toggle('disabled', !isPtt);
      currentPttKey = cfg.ptt_key || 'Space';
      pttKeyBtn.textContent = currentPttKey;
      updatePttStatus(false);
    }

    autoSelectAndPopulateMic(inputDevs, cfg ? cfg.selected_mic : null);
    autoSelectAndPopulateSpeaker(outputDevs, cfg ? cfg.selected_speaker : null);

    // Virtual Cable Status
    try {
      const vStatus = await invoke('get_virtual_status');
      if (vStatus && vStatus.is_installed) {
        virtualCableBadge.textContent = 'Installed & Ready';
        virtualCableBadge.style.background = 'var(--accent-green-dim)';
        virtualCableBadge.style.color = 'var(--accent-green)';
      } else {
        virtualCableBadge.textContent = 'Driver Required';
        virtualCableBadge.style.background = 'var(--accent-amber-dim)';
        virtualCableBadge.style.color = 'var(--accent-amber)';
      }
    } catch (e) {}

    // Live test status
    try {
      const liveStatus = await invoke('get_live_test_status');
      updateLiveTestUI(liveStatus.is_active);
    } catch (e) {}

    // Check URL parameters or hash for theme and tab overrides (useful for preview & testing)
    try {
      const rawQuery = window.location.search || (window.location.hash ? window.location.hash.substring(1) : '');
      const urlParams = new URLSearchParams(rawQuery);
      const themeParam = urlParams.get('theme');
      if (themeParam) applyTheme(themeParam);
      const tabParam = urlParams.get('tab');
      if (tabParam) {
        tabButtons.forEach((b) => {
          const isSelected = b.dataset.tab === tabParam;
          b.classList.toggle('active', isSelected);
          b.setAttribute('aria-selected', isSelected ? 'true' : 'false');
        });
        tabPanels.forEach((p) => {
          p.classList.toggle('active', p.id === tabParam);
        });
      }
    } catch (_) {}
  } catch (err) {
    console.error('Initialization error:', err);
  }

  // Start polling
  startPolling();
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
