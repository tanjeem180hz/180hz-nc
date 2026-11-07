// QuietMic Vanilla JS UI Controller
const invoke = window.__TAURI__ ? (window.__TAURI__.core ? window.__TAURI__.core.invoke : window.__TAURI__.invoke) : null;

// UI Elements
const cpuIndicator = document.getElementById('cpuIndicator');
const bypassBtn = document.getElementById('bypassBtn');
const vbcableBanner = document.getElementById('vbcableBanner');
const downloadCableBtn = document.getElementById('downloadCableBtn');
const recheckCableBtn = document.getElementById('recheckCableBtn');
const modeButtons = document.querySelectorAll('.mode-btn');
const strengthSlider = document.getElementById('strengthSlider');
const strengthValue = document.getElementById('strengthValue');
const micSelect = document.getElementById('micSelect');
const vbcableStatus = document.getElementById('vbcableStatus');
const inputMeter = document.getElementById('inputMeter');
const outputMeter = document.getElementById('outputMeter');
const inputPeakText = document.getElementById('inputPeakText');
const outputPeakText = document.getElementById('outputPeakText');
const aecToggle = document.getElementById('aecToggle');
const comfortNoiseToggle = document.getElementById('comfortNoiseToggle');
const speakerDenoiseToggle = document.getElementById('speakerDenoiseToggle');
const autostartToggle = document.getElementById('autostartToggle');

// Live Test UI Elements
const liveTestToggleBtn = document.getElementById('liveTestToggleBtn');
const liveTestBtnIcon = document.getElementById('liveTestBtnIcon');
const liveTestBtnText = document.getElementById('liveTestBtnText');
const liveBadge = document.getElementById('liveBadge');
const feedbackAlert = document.getElementById('feedbackAlert');
const monitorSelect = document.getElementById('monitorSelect');
const abCleanedBtn = document.getElementById('abCleanedBtn');
const abOriginalBtn = document.getElementById('abOriginalBtn');
const monitorVolumeSlider = document.getElementById('monitorVolumeSlider');
const monitorVolumeValue = document.getElementById('monitorVolumeValue');
const startRecordBtn = document.getElementById('startRecordBtn');
const playReplayBtn = document.getElementById('playReplayBtn');
const exportWavBtn = document.getElementById('exportWavBtn');
const recordStatusLabel = document.getElementById('recordStatusLabel');
const recordProgressBar = document.getElementById('recordProgressBar');
const wavExportNotice = document.getElementById('wavExportNotice');

let currentConfig = {
  mode: 'Standard',
  strength: 1.0,
  bypass: false,
  aec_enabled: false,
  speaker_denoise: false,
  comfort_noise: true,
  selected_mic: null,
  start_with_windows: false,
};

let metricsTimer = null;
let deviceTimer = null;

async function pollDevices() {
  if (!invoke) return;
  try {
    const inputDevices = await invoke('get_input_devices');
    const prevMic = micSelect.value;
    populateInputDeviceList(inputDevices);
    if (prevMic) micSelect.value = prevMic;

    const outputDevices = await invoke('get_output_devices');
    const prevMonitor = monitorSelect.value;
    populateOutputDeviceList(outputDevices);
    if (prevMonitor) monitorSelect.value = prevMonitor;

    const virtualStatus = await invoke('get_virtual_status');
    updateVirtualStatus(virtualStatus);
  } catch (e) {}
}

function startPolling() {
  if (!metricsTimer) {
    metricsTimer = setInterval(updateMetrics, 66);
  }
  if (!deviceTimer) {
    deviceTimer = setInterval(pollDevices, 3000);
  }
}

function stopPolling() {
  if (metricsTimer) {
    clearInterval(metricsTimer);
    metricsTimer = null;
  }
  if (deviceTimer) {
    clearInterval(deviceTimer);
    deviceTimer = null;
  }
}

// Pause timers and stop live test when window is hidden or page unloaded
document.addEventListener('visibilitychange', () => {
  if (document.hidden) {
    stopPolling();
  } else {
    updateMetrics();
    pollDevices();
    startPolling();
  }
});

window.addEventListener('beforeunload', () => {
  if (invoke) {
    invoke('stop_live_test');
  }
});

window.addEventListener('pagehide', () => {
  stopPolling();
  if (invoke) {
    invoke('stop_live_test');
  }
});

window.addEventListener('pageshow', startPolling);

async function init() {
  if (!invoke) {
    console.log("Running in preview/browser mode without Tauri IPC");
    startMeterSimulation();
    return;
  }

  try {
    // 1. Fetch devices and configuration
    await pollDevices();

    currentConfig = await invoke('get_config');
    applyConfigToUI(currentConfig);

    // Initial query of Live Test status
    const liveStatus = await invoke('get_live_test_status');
    updateLiveTestUI(liveStatus);

    // 2. Start polling only if window is currently visible
    if (!document.hidden) {
      startPolling();
    }
  } catch (err) {
    console.error("Initialization error:", err);
  }
}

function populateInputDeviceList(devices) {
  micSelect.innerHTML = '<option value="">Default System Microphone</option>';
  devices.forEach(dev => {
    const opt = document.createElement('option');
    opt.value = dev.name;
    opt.textContent = `${dev.name} (${dev.default_sample_rate / 1000} kHz)`;
    if (dev.is_default) {
      opt.textContent += ' [Default]';
    }
    micSelect.appendChild(opt);
  });
}

function populateOutputDeviceList(devices) {
  monitorSelect.innerHTML = '<option value="">Default System Headphones / Speakers</option>';
  devices.forEach(dev => {
    // Strictly filter out any virtual cables just in case
    const nameLower = dev.name.toLowerCase();
    if (nameLower.includes('cable') || nameLower.includes('vb-audio')) {
      return;
    }
    const opt = document.createElement('option');
    opt.value = dev.name;
    opt.textContent = `${dev.name} (${dev.default_sample_rate / 1000} kHz)`;
    if (dev.is_default) {
      opt.textContent += ' [Default]';
    }
    monitorSelect.appendChild(opt);
  });
}

function updateVirtualStatus(status) {
  if (status.is_installed) {
    vbcableBanner.classList.add('hidden');
    vbcableStatus.className = 'status-online';
    vbcableStatus.textContent = '• CABLE Output (Ready)';
  } else {
    vbcableBanner.classList.remove('hidden');
    vbcableStatus.className = 'status-offline';
    vbcableStatus.textContent = '⚠️ VB-CABLE Not Found';
  }
}

function applyConfigToUI(cfg) {
  // Mode buttons
  modeButtons.forEach(btn => {
    btn.classList.toggle('active', btn.dataset.mode === cfg.mode);
  });

  // Strength
  const pct = Math.round(cfg.strength * 100);
  strengthSlider.value = pct;
  strengthValue.textContent = `${pct}%`;

  // Bypass
  updateBypassUI(cfg.bypass);

  // Toggles
  aecToggle.checked = cfg.aec_enabled;
  comfortNoiseToggle.checked = cfg.comfort_noise;
  speakerDenoiseToggle.checked = cfg.speaker_denoise;
  autostartToggle.checked = cfg.start_with_windows;

  if (cfg.selected_mic) {
    micSelect.value = cfg.selected_mic;
  }
}

function updateBypassUI(isBypassed) {
  if (isBypassed) {
    bypassBtn.textContent = 'BYPASSED';
    bypassBtn.className = 'badge-btn bypassed';
  } else {
    bypassBtn.textContent = 'ACTIVE';
    bypassBtn.className = 'badge-btn active';
  }
}

function updateLiveTestUI(st) {
  if (!st) return;

  if (st.is_active) {
    liveBadge.classList.remove('hidden');
    liveTestToggleBtn.classList.add('btn-live-active');
    liveTestBtnIcon.textContent = '⏹';
    liveTestBtnText.textContent = 'Stop Live Test';
  } else {
    liveBadge.classList.add('hidden');
    liveTestToggleBtn.classList.remove('btn-live-active');
    liveTestBtnIcon.textContent = '▶';
    liveTestBtnText.textContent = 'Start Live Test';
  }

  // A/B mode
  abCleanedBtn.classList.toggle('active', st.ab_mode === 'Cleaned');
  abOriginalBtn.classList.toggle('active', st.ab_mode === 'Original');

  // Howling / Feedback alert
  if (st.feedback_detected) {
    feedbackAlert.classList.remove('hidden');
  } else {
    feedbackAlert.classList.add('hidden');
  }

  // Record & Replay
  if (st.is_recording) {
    const pct = Math.min(100, (st.recording_seconds / 10.0) * 100);
    recordProgressBar.style.width = `${pct}%`;
    recordStatusLabel.textContent = `Recording... (${st.recording_seconds.toFixed(1)}s / 10s)`;
    startRecordBtn.disabled = true;
    playReplayBtn.disabled = true;
    exportWavBtn.disabled = true;
  } else if (st.is_replaying) {
    recordProgressBar.style.width = '100%';
    recordStatusLabel.textContent = st.replay_is_cleaned ? 'Replaying: Cleaned...' : 'Replaying: Original (Raw)...';
    startRecordBtn.disabled = true;
    playReplayBtn.disabled = true;
    playReplayBtn.textContent = 'Replaying...';
  } else {
    playReplayBtn.textContent = 'Play Replay (A/B)';
    startRecordBtn.disabled = false;
    if (st.has_recorded_audio) {
      recordProgressBar.style.width = '100%';
      recordStatusLabel.textContent = '10s Sample Ready';
      playReplayBtn.disabled = false;
      exportWavBtn.disabled = false;
    } else {
      recordProgressBar.style.width = '0%';
      recordStatusLabel.textContent = 'Idle';
      playReplayBtn.disabled = true;
      exportWavBtn.disabled = true;
    }
  }
}

async function updateMetrics() {
  if (!invoke) return;
  try {
    const m = await invoke('get_metrics');
    // Frame time in microseconds -> converted to milliseconds
    const frameMs = (m.frame_time_us / 1000).toFixed(1);
    // Estimated CPU %: (time / 10,000 us frame) * 100 on single core
    const cpuPct = Math.min(100, (m.frame_time_us / 10000) * 100).toFixed(1);
    cpuIndicator.textContent = `CPU: ${cpuPct}% (${frameMs}ms)`;

    // Map -60 dB to 0 dB into 0% - 100% meter width
    const inPct = Math.max(0, Math.min(100, ((m.input_peak_db + 60) / 60) * 100));
    const outPct = Math.max(0, Math.min(100, ((m.output_peak_db + 60) / 60) * 100));

    inputMeter.style.width = `${inPct}%`;
    outputMeter.style.width = `${outPct}%`;

    inputPeakText.textContent = `${Math.round(m.input_peak_db)} dB`;
    outputPeakText.textContent = `${Math.round(m.output_peak_db)} dB`;

    // Query Live Test status
    const liveStatus = await invoke('get_live_test_status');
    updateLiveTestUI(liveStatus);
  } catch (err) {
    // Throttled logging to avoid spamming
  }
}

// Event Listeners
bypassBtn.addEventListener('click', async () => {
  currentConfig.bypass = !currentConfig.bypass;
  updateBypassUI(currentConfig.bypass);
  if (invoke) await invoke('set_bypass', { bypass: currentConfig.bypass });
});

modeButtons.forEach(btn => {
  btn.addEventListener('click', async () => {
    const mode = btn.dataset.mode;
    modeButtons.forEach(b => b.classList.toggle('active', b === btn));
    currentConfig.mode = mode;
    if (invoke) await invoke('set_mode', { mode });
  });
});

strengthSlider.addEventListener('input', async (e) => {
  const val = parseInt(e.target.value, 10);
  strengthValue.textContent = `${val}%`;
  currentConfig.strength = val / 100;
  if (invoke) await invoke('set_strength', { strength: currentConfig.strength });
});

micSelect.addEventListener('change', async (e) => {
  const selected = e.target.value || null;
  currentConfig.selected_mic = selected;
  if (invoke) await invoke('set_input_device', { name: selected });
});

aecToggle.addEventListener('change', async (e) => {
  currentConfig.aec_enabled = e.target.checked;
  if (invoke) await invoke('set_aec', { enabled: e.target.checked });
});

comfortNoiseToggle.addEventListener('change', async (e) => {
  currentConfig.comfort_noise = e.target.checked;
  if (invoke) await invoke('set_comfort_noise', { enabled: e.target.checked });
});

speakerDenoiseToggle.addEventListener('change', async (e) => {
  currentConfig.speaker_denoise = e.target.checked;
  if (invoke) await invoke('set_speaker_denoise', { enabled: e.target.checked });
});

autostartToggle.addEventListener('change', async (e) => {
  currentConfig.start_with_windows = e.target.checked;
  if (invoke) await invoke('set_autostart', { enabled: e.target.checked });
});

downloadCableBtn.addEventListener('click', () => {
  if (invoke) {
    invoke('open_url', { url: 'https://vb-audio.com/Cable/' });
  } else {
    window.open('https://vb-audio.com/Cable/', '_blank');
  }
});

recheckCableBtn.addEventListener('click', async () => {
  if (invoke) {
    const status = await invoke('get_virtual_status');
    updateVirtualStatus(status);
  }
});

// Live Test Listeners
liveTestToggleBtn.addEventListener('click', async () => {
  if (!invoke) return;
  try {
    await invoke('toggle_live_test', { deviceName: monitorSelect.value || null });
    const st = await invoke('get_live_test_status');
    updateLiveTestUI(st);
  } catch (err) {
    alert("Live Test error: " + err);
  }
});

monitorSelect.addEventListener('change', async () => {
  if (!invoke) return;
  const isCurrentlyActive = liveTestToggleBtn.classList.contains('btn-live-active');
  if (isCurrentlyActive) {
    try {
      await invoke('start_live_test', { deviceName: monitorSelect.value || null });
    } catch (err) {
      alert("Failed to switch monitor device: " + err);
    }
  }
});

abCleanedBtn.addEventListener('click', async () => {
  abCleanedBtn.classList.add('active');
  abOriginalBtn.classList.remove('active');
  if (invoke) await invoke('set_live_test_ab', { mode: 'Cleaned' });
});

abOriginalBtn.addEventListener('click', async () => {
  abOriginalBtn.classList.add('active');
  abCleanedBtn.classList.remove('active');
  if (invoke) await invoke('set_live_test_ab', { mode: 'Original' });
});

monitorVolumeSlider.addEventListener('input', async (e) => {
  const val = parseInt(e.target.value, 10);
  monitorVolumeValue.textContent = `${val}%`;
  if (invoke) await invoke('set_live_test_volume', { volume: val / 100 });
});

startRecordBtn.addEventListener('click', async () => {
  if (!invoke) return;
  try {
    const isCurrentlyActive = liveTestToggleBtn.classList.contains('btn-live-active');
    if (!isCurrentlyActive) {
      await invoke('start_live_test', { deviceName: monitorSelect.value || null });
    }
    await invoke('start_record_and_replay');
    wavExportNotice.classList.add('hidden');
  } catch (err) {
    alert("Recording error: " + err);
  }
});

playReplayBtn.addEventListener('click', async () => {
  if (!invoke) return;
  try {
    const isCurrentlyActive = liveTestToggleBtn.classList.contains('btn-live-active');
    if (!isCurrentlyActive) {
      await invoke('start_live_test', { deviceName: monitorSelect.value || null });
    }
    await invoke('start_replay_playback');
  } catch (err) {
    alert("Replay error: " + err);
  }
});

exportWavBtn.addEventListener('click', async () => {
  if (!invoke) return;
  try {
    const [rawPath, cleanPath] = await invoke('export_recorded_wav', { dir: null });
    wavExportNotice.classList.remove('hidden');
    wavExportNotice.innerHTML = `Saved WAV files:<br>&bull; Raw: <code>${rawPath}</code><br>&bull; Cleaned: <code>${cleanPath}</code>`;
  } catch (err) {
    alert("Export WAV error: " + err);
  }
});

const runDiagBtn = document.getElementById('runDiagBtn');
const diagResults = document.getElementById('diagResults');
const diagList = document.getElementById('diagList');

if (runDiagBtn) {
  runDiagBtn.addEventListener('click', async () => {
    if (!invoke) return;
    runDiagBtn.disabled = true;
    runDiagBtn.textContent = 'Running diagnostics...';
    try {
      const diag = await invoke('get_system_audio_diagnostics');
      diagList.innerHTML = '';

      const micStatus = diag.mic_permission_granted ? '✅ Microphone Permission: Granted' : '❌ Microphone Permission: Denied in Windows Settings';
      const cableStatus = diag.virtual_cable_installed ? '✅ Virtual Device: VB-CABLE Detected' : '⚠️ Virtual Device: VB-CABLE Not Found';
      const inputStatus = `🎙️ Default Input: ${diag.default_input_name} (${diag.default_input_sample_rate} Hz)`;
      const outputStatus = `🔊 Default Output: ${diag.default_output_name} (${diag.default_output_sample_rate} Hz)`;

      [micStatus, cableStatus, inputStatus, outputStatus].forEach(text => {
        const li = document.createElement('li');
        li.className = 'diag-item';
        li.textContent = text;
        diagList.appendChild(li);
      });

      diag.recommendations.forEach(rec => {
        const li = document.createElement('li');
        li.className = 'diag-item';
        li.innerHTML = `<strong>Tip:</strong> ${rec}`;
        diagList.appendChild(li);
      });

      diagResults.classList.remove('hidden');
    } catch (err) {
      alert("Diagnostic error: " + err);
    } finally {
      runDiagBtn.disabled = false;
      runDiagBtn.textContent = '🔍 Run Audio Diagnostics';
    }
  });
}

// Demo fallback when opened in a regular web browser
function startMeterSimulation() {
  setInterval(() => {
    const inVal = Math.random() * 80;
    const outVal = inVal * 0.4;
    inputMeter.style.width = `${inVal}%`;
    outputMeter.style.width = `${outVal}%`;
    inputPeakText.textContent = `${Math.round(-60 + (inVal / 100) * 60)} dB`;
    outputPeakText.textContent = `${Math.round(-60 + (outVal / 100) * 60)} dB`;
  }, 100);
}

document.addEventListener('DOMContentLoaded', init);
