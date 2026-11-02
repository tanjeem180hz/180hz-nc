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

async function init() {
  if (!invoke) {
    console.log("Running in preview/browser mode without Tauri IPC");
    startMeterSimulation();
    return;
  }

  try {
    // 1. Fetch devices and configuration
    const devices = await invoke('get_input_devices');
    populateDeviceList(devices);

    const virtualStatus = await invoke('get_virtual_status');
    updateVirtualStatus(virtualStatus);

    currentConfig = await invoke('get_config');
    applyConfigToUI(currentConfig);

    // 2. Start polling metrics at ~15 fps (every 66ms)
    setInterval(updateMetrics, 66);
  } catch (err) {
    console.error("Initialization error:", err);
  }
}

function populateDeviceList(devices) {
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
