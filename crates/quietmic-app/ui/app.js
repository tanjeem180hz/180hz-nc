// -------------------------------------------------------------
// 180hz nc - Audio App UI Controller
// Classic Card Layout, High DPI, Dark/Light Themes, 0% Idle CPU
// -------------------------------------------------------------

function getInvoke() {
  if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
    return window.__TAURI__.core.invoke;
  }
  if (window.__TAURI__ && typeof window.__TAURI__.invoke === 'function') {
    return window.__TAURI__.invoke;
  }
  console.warn('Tauri API not detected; using mock fallback.');
  return mockInvoke;
}

async function invoke(cmd, args) {
  const invoker = getInvoke();
  return invoker(cmd, args);
}

// Mock fallback for standalone browser preview / unit testing
function mockInvoke(cmd, args) {
  switch (cmd) {
    case 'get_config':
      return Promise.resolve({
        mode: 'MaxQuality',
        strength: 1.0,
        bypass: false,
        aec_enabled: false,
        mic_volume: 1.0,
        speaker_volume: 1.0,
        auto_sensitivity: true,
        sensitivity_threshold: -50.0,
        push_to_talk: false,
        ptt_key: 'Space',
        theme: 'dark',
      });
    case 'get_input_devices':
      return Promise.resolve([
        { id: 'default', name: 'Microphone Array (Realtek Audio)', is_default: true },
        { id: 'cable_out', name: 'CABLE Output (VB-Audio Virtual Cable)', is_default: false },
      ]);
    case 'get_output_devices':
      return Promise.resolve([
        { id: 'default', name: 'Speakers / Headphones (Realtek Audio)', is_default: true },
        { id: 'cable_in', name: 'CABLE Input (VB-Audio Virtual Cable)', is_default: false },
      ]);
    case 'get_metrics':
      return Promise.resolve({
        input_peak_db: -42.0,
        output_peak_db: -55.0,
        clipping_count: 0,
        noise_reduction_db: 24.5,
        frames_processed: 1240,
        frames_passthrough: 0,
        dsp_latency_us: 185,
        aec_endpoint: 'Speakers',
        loop_warning: false,
        cpu_usage_pct: 0.2,
      });
    case 'get_live_test_status':
      return Promise.resolve({ is_active: false, volume: 1.0, output_device: null, mode: 'Cleaned' });
    case 'toggle_live_test':
      return Promise.resolve(true);
    case 'set_live_test_ab':
    case 'set_noise_mode':
    case 'set_strength':
    case 'set_bypass':
    case 'set_mic_volume':
    case 'set_speaker_volume':
    case 'set_auto_sensitivity':
    case 'set_sensitivity_threshold':
    case 'set_aec':
    case 'set_push_to_talk':
    case 'set_ptt_key':
    case 'set_ptt_state':
    case 'set_theme':
    case 'set_close_to_tray':
    case 'set_start_with_windows':
    case 'open_windows_startup_settings':
    case 'open_windows_taskbar_settings':
      return Promise.resolve();
    case 'get_autostart_status':
      return Promise.resolve({ enabled: false, disabled_by_os: false });
    case 'run_pipeline_self_test_command':
      return Promise.resolve({
        success: true,
        snr_reduction_db: 28.4,
        speech_preservation_delta_db: 0.15,
        latency_us: 182,
        passthrough_frames: 0,
        message: 'Pipeline DSP operational with 0 pass-through frames and 28.4 dB noise reduction.',
      });
    case 'run_echo_doctor_command':
      return Promise.resolve({
        loop_severity: 'CLEAN',
        plain_explanation: 'All Windows playback endpoints and communications routes are correctly configured. No feedback loop detected.',
        recommended_actions: ['Routing is clean and verified.'],
        discord_tips: [
          'Share an Application Window instead of Entire Screen so Discord captures audio only from that app.',
          'In Discord Voice & Video settings, set Input to CABLE Output and Output to your Headphones.',
        ],
      });
    case 'run_aec_speaker_test_command':
      return Promise.resolve({ success: true, message: 'AEC chirp calibration complete. Delay estimated: 42 ms.' });
    default:
      return Promise.resolve(null);
  }
}

// -------------------------------------------------------------
// DOM Elements
// -------------------------------------------------------------
const masterPowerBtn = document.getElementById('masterPowerBtn');
const masterPowerText = document.getElementById('masterPowerText');
const themeToggleBtn = document.getElementById('themeToggleBtn');
const themeToggleIcon = document.getElementById('themeToggleIcon');
const themeSelector = document.getElementById('themeSelector');
const prefsThemeSelector = document.getElementById('prefsThemeSelector');
const engineStatusPill = document.getElementById('engineStatusPill');
const engineStatusText = document.getElementById('engineStatusText');

const micSelect = document.getElementById('micSelect');
const micAutoBadge = document.getElementById('micAutoBadge');
const micVolumeSlider = document.getElementById('micVolumeSlider');
const micVolText = document.getElementById('micVolText');
const micMeterFill = document.getElementById('micMeterFill');
const micMeterDb = document.getElementById('micMeterDb');
const clippingIndicator = document.getElementById('clippingIndicator');

const speakerSelect = document.getElementById('speakerSelect');
const spkAutoBadge = document.getElementById('spkAutoBadge');
const speakerVolumeSlider = document.getElementById('speakerVolumeSlider');
const speakerVolText = document.getElementById('speakerVolText');

const liveVoiceBtn = document.getElementById('liveVoiceBtn');
const liveVoiceIconOff = document.getElementById('liveVoiceIconOff');
const liveVoiceIconOn = document.getElementById('liveVoiceIconOn');
const liveVoiceTitle = document.getElementById('liveVoiceTitle');
const liveVoiceDesc = document.getElementById('liveVoiceDesc');
const abCleanedBtn = document.getElementById('abCleanedBtn');
const abOriginalBtn = document.getElementById('abOriginalBtn');

const noiseReductionBadge = document.getElementById('noiseReductionBadge');
const segButtons = document.querySelectorAll('.seg-btn');
const strengthSlider = document.getElementById('strengthSlider');
const strengthValText = document.getElementById('strengthValText');
const framesProcessedText = document.getElementById('framesProcessedText');
const framesPassthroughText = document.getElementById('framesPassthroughText');
const dspLatencyText = document.getElementById('dspLatencyText');
const pipelineSelfTestBtn = document.getElementById('pipelineSelfTestBtn');

const autoSensitivityToggle = document.getElementById('autoSensitivityToggle');
const manualSensitivitySection = document.getElementById('manualSensitivitySection');
const sensitivitySlider = document.getElementById('sensitivitySlider');
const sensitivityText = document.getElementById('sensitivityText');

const aecToggle = document.getElementById('aecToggle');
const aecDetails = document.getElementById('aecDetails');
const aecEndpointBadge = document.getElementById('aecEndpointBadge');
const loopWarningPill = document.getElementById('loopWarningPill');
const aecRefDeviceText = document.getElementById('aecRefDeviceText');
const aecRefLevelText = document.getElementById('aecRefLevelText');
const aecStatusText = document.getElementById('aecStatusText');
const aecErleText = document.getElementById('aecErleText');
const aecDelayText = document.getElementById('aecDelayText');
const aecFramesText = document.getElementById('aecFramesText');
const aecTestBtn = document.getElementById('aecTestBtn');
const echoDoctorBtn = document.getElementById('echoDoctorBtn');

const pttToggle = document.getElementById('pttToggle');
const pttControls = document.getElementById('pttControls');
const pttKeyBtn = document.getElementById('pttKeyBtn');
const pttStatusBadge = document.getElementById('pttStatusBadge');

const footerStatusText = document.getElementById('footerStatusText');
const footerCpuText = document.getElementById('footerCpuText');
const footerBufferText = document.getElementById('footerBufferText');

// Preferences
const closeToTrayToggle = document.getElementById('closeToTrayToggle');
const startWithWindowsToggle = document.getElementById('startWithWindowsToggle');
const reduceEffectsToggle = document.getElementById('reduceEffectsToggle');
let isReduceEffectsActive = false;
let userManuallySetReduceEffects = false;
const startupWarning = document.getElementById('startupWarning');
const openStartupSettingsBtn = document.getElementById('openStartupSettingsBtn');
const togglePinGuideBtn = document.getElementById('togglePinGuideBtn');
const pinGuideDetails = document.getElementById('pinGuideDetails');
const openTaskbarSettingsBtn = document.getElementById('openTaskbarSettingsBtn');

// First-time Tray Toast
const trayToast = document.getElementById('trayToast');
const dismissTrayToastBtn = document.getElementById('dismissTrayToastBtn');

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

const pipelineModal = document.getElementById('pipelineModal');
const closePipelineModalBtn = document.getElementById('closePipelineModalBtn');
const dismissPipelineModalBtn = document.getElementById('dismissPipelineModalBtn');
const pipelineResultBanner = document.getElementById('pipelineResultBanner');
const pipelineResultIcon = document.getElementById('pipelineResultIcon');
const pipelineResultText = document.getElementById('pipelineResultText');
const pipelineDetails = document.getElementById('pipelineDetails');

// -------------------------------------------------------------
// Application State
// -------------------------------------------------------------
let currentConfig = null;
let currentPttKey = 'Space';
let isRecordingKey = false;
let isPttTransmitting = false;
let isBypassed = false;
let currentMode = 'MaxQuality';
let currentTheme = 'dark';
let isLiveMonitoring = false;
let liveStatusPollCount = 0;
let lastClippingCount = 0;
let clipTimer = null;
let pollIntervalId = null;

// -------------------------------------------------------------
// Initialization
// -------------------------------------------------------------
async function init() {
  try {
    const [cfg, inputDevs, outputDevs] = await Promise.all([
      invoke('get_config'),
      invoke('get_input_devices'),
      invoke('get_output_devices'),
    ]);

    currentConfig = cfg;
    const urlParams = new URLSearchParams(window.location.search);
    const queryTheme = urlParams.get('theme');
    const hashTheme = window.location.hash.includes('light') || window.location.hash.includes('daylight') ? 'light' : (window.location.hash.includes('dark') ? 'dark' : null);
    let savedChoice = null;
    try {
      savedChoice = localStorage.getItem('quietmic_theme_choice');
    } catch (e) {}
    applyTheme(queryTheme || hashTheme || savedChoice || cfg.theme || 'classic-dark');
    initReduceEffects();
    if (urlParams.get('reduce_effects') === 'true' || window.location.hash.includes('reduce-effects')) {
      setReduceEffects(true);
    }
    populateDevices(inputDevs || [], outputDevs || [], cfg);
    applyConfig(cfg);

    const queryTab = urlParams.get('tab') || (window.location.hash.includes('tab-') ? window.location.hash.split('tab-')[1] : null);
    if (queryTab) {
      const paneName = 'pane' + queryTab.charAt(0).toUpperCase() + queryTab.slice(1);
      const targetBtn = document.querySelector(`[data-tab="${paneName}"]`);
      if (targetBtn) targetBtn.click();
    }

    if (urlParams.get('modal') === 'echo-doctor' || window.location.hash.includes('echo-doctor')) {
      if (echoDoctorBtn) echoDoctorBtn.click();
      else if (echoDoctorModal) echoDoctorModal.style.display = 'flex';
    } else if (urlParams.get('modal') === 'pipeline' || window.location.hash.includes('pipeline')) {
      if (pipelineSelfTestBtn) pipelineSelfTestBtn.click();
      else if (pipelineModal) pipelineModal.style.display = 'flex';
    } else if (urlParams.get('toast') === 'tray' || window.location.hash.includes('toast')) {
      showTrayToastNotification();
    }

    const liveStatus = await invoke('get_live_test_status').catch(() => null);
    if (liveStatus) {
      updateLiveTestUI(liveStatus.is_active);
      if (abOriginalBtn && abCleanedBtn) {
        if (liveStatus.mode === 'Original') {
          abOriginalBtn.classList.add('active');
          abCleanedBtn.classList.remove('active');
        } else {
          abCleanedBtn.classList.add('active');
          abOriginalBtn.classList.remove('active');
        }
      }
    }

    try {
      const autostart = await invoke('get_autostart_status');
      if (autostart) {
        if (startWithWindowsToggle) {
          startWithWindowsToggle.checked = !!autostart.enabled;
        }
        if (startupWarning) {
          startupWarning.style.display = autostart.disabled_by_os ? 'inline-block' : 'none';
        }
      }
    } catch (e) {
      console.warn('Failed to query autostart status:', e);
    }

    if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.listen === 'function') {
      window.__TAURI__.event.listen('show-tray-toast', () => {
        showTrayToastNotification();
      });
    }
  } catch (err) {
    console.error('Initialization error:', err);
  }

  setupEventListeners();
  startMetricsPolling();
  setupCpuThrottling();
}

// -------------------------------------------------------------
// Reduce Effects Setting & Fallback Logic
// -------------------------------------------------------------
function setReduceEffects(enabled, reason) {
  isReduceEffectsActive = enabled;
  if (reduceEffectsToggle) reduceEffectsToggle.checked = enabled;
  if (enabled) {
    document.documentElement.setAttribute('data-reduce-effects', 'true');
    document.body.classList.add('reduce-effects');
  } else {
    document.documentElement.removeAttribute('data-reduce-effects');
    document.body.classList.remove('reduce-effects');
  }
  try {
    localStorage.setItem('reduce_effects', enabled ? 'true' : 'false');
  } catch (e) {}

  if (reason) {
    console.info(`[180hz nc] Reduce effects auto-engaged: ${reason}`);
    showStatusNotice(`Reduce effects active (${reason})`);
  }
}

function showStatusNotice(message) {
  if (footerStatusText) {
    const prev = footerStatusText.textContent;
    footerStatusText.textContent = message;
    setTimeout(() => {
      footerStatusText.textContent = prev;
    }, 4000);
  }
}

function initReduceEffects() {
  let saved = null;
  try {
    saved = localStorage.getItem('reduce_effects');
  } catch (e) {}

  if (saved !== null) {
    userManuallySetReduceEffects = true;
    setReduceEffects(saved === 'true');
  } else {
    // Check backdrop-filter support
    const supported = window.CSS && (CSS.supports('backdrop-filter', 'blur(12px)') || CSS.supports('-webkit-backdrop-filter', 'blur(12px)'));
    if (!supported) {
      setReduceEffects(true, 'Backdrop filter unsupported');
      return;
    }
    // Check software GPU
    try {
      const c = document.createElement('canvas');
      const gl = c.getContext('webgl') || c.getContext('experimental-webgl');
      if (gl) {
        const dbg = gl.getExtension('WEBGL_debug_renderer_info');
        if (dbg) {
          const rend = gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) || '';
          if (/swiftshader|llvmpipe|software|basic render/i.test(rend)) {
            setReduceEffects(true, 'Software rendering detected');
            return;
          }
        }
      }
    } catch (e) {}
  }

  // Monitor frame budget: if frame time > 24ms for ~2s continuously, auto-enable
  let slowFrames = 0;
  let lastT = performance.now();
  function checkPerf(now) {
    if (!document.hidden && !isReduceEffectsActive && !userManuallySetReduceEffects) {
      const dt = now - lastT;
      if (dt > 24.0) {
        slowFrames++;
        if (slowFrames > 60) {
          setReduceEffects(true, 'Frame rate budget exceeded (>24ms)');
          return;
        }
      } else {
        slowFrames = Math.max(0, slowFrames - 1);
      }
    }
    lastT = now;
    requestAnimationFrame(checkPerf);
  }
  requestAnimationFrame(checkPerf);
}

// -------------------------------------------------------------
// Theme Management (Dual Theme Switcher: Classic Dark & Soft Glass)
// -------------------------------------------------------------
let currentThemeMode = 'classic-dark';

function applyTheme(theme) {
  let normalized = theme;
  if (!theme || theme === 'night-lab' || theme === 'dark' || theme === 'classic') {
    normalized = 'classic-dark';
  } else if (theme === 'light') {
    normalized = 'classic-light';
  } else if (theme === 'amber-console') {
    normalized = 'soft-glass-dark';
  } else if (theme === 'daylight') {
    normalized = 'soft-glass-light';
  }

  currentThemeMode = normalized;
  const isSoftGlass = normalized.startsWith('soft-glass');
  const isLight = normalized.endsWith('light');

  // 1. Swap stylesheet href cleanly
  const themeStylesheet = document.getElementById('themeStylesheet');
  if (themeStylesheet) {
    const targetHref = isSoftGlass ? 'theme-soft-glass.css' : 'style.css';
    const currentHref = themeStylesheet.getAttribute('href') || '';
    if (!currentHref.endsWith(targetHref)) {
      themeStylesheet.setAttribute('href', targetHref);
    }
  }

  // 2. Set DOM attributes on html element
  const colorMode = isLight ? 'light' : 'dark';
  const uiMode = isSoftGlass ? 'soft-glass' : 'classic';
  document.documentElement.setAttribute('data-theme', colorMode);
  document.documentElement.setAttribute('data-ui-mode', uiMode);

  // 3. Update header branding subtitle
  const brandSubtitle = document.getElementById('brandSubtitle');
  if (brandSubtitle) {
    brandSubtitle.textContent = isSoftGlass ? 'Soft Glass Workspace' : 'Voice Clarity & Noise Removal';
  }

  // 4. Update dropdowns
  if (themeSelector) themeSelector.value = normalized;
  if (prefsThemeSelector) prefsThemeSelector.value = normalized;

  // 5. Update toggle button icon and title
  if (themeToggleIcon) {
    themeToggleIcon.textContent = isLight ? '☀️' : '🌙';
  }
  if (themeToggleBtn) {
    themeToggleBtn.title = isSoftGlass
      ? (isLight ? 'Switch to Soft Glass Dark' : 'Switch to Soft Glass Light')
      : (isLight ? 'Switch to Classic Dark' : 'Switch to Classic Light');
  }

  // 6. Manage tab states when in Soft Glass
  if (isSoftGlass) {
    const activeTab = document.querySelector('.tab-btn.active');
    const targetPaneId = activeTab ? activeTab.getAttribute('data-tab') : 'paneAudio';
    document.querySelectorAll('.tab-pane').forEach((p) => {
      if (p.id === targetPaneId) {
        p.classList.remove('hidden');
      } else {
        p.classList.add('hidden');
      }
    });
  }

  try {
    localStorage.setItem('quietmic_theme_choice', normalized);
  } catch (e) {}
}

function setThemeAndSave(themeName) {
  applyTheme(themeName);
  let backendTheme = 'dark';
  if (themeName === 'classic-dark') backendTheme = 'dark';
  else if (themeName === 'classic-light') backendTheme = 'light';
  else if (themeName === 'soft-glass-dark') backendTheme = 'amber-console';
  else if (themeName === 'soft-glass-light') backendTheme = 'daylight';

  invoke('set_theme', { theme: backendTheme }).catch((e) => console.error('Failed to save theme:', e));
}

function toggleTheme() {
  let nextTheme;
  if (currentThemeMode === 'classic-dark') {
    nextTheme = 'soft-glass-dark';
  } else if (currentThemeMode === 'soft-glass-dark') {
    nextTheme = 'classic-dark';
  } else if (currentThemeMode === 'soft-glass-light') {
    nextTheme = 'soft-glass-dark';
  } else {
    nextTheme = 'classic-dark';
  }
  setThemeAndSave(nextTheme);
}

// -------------------------------------------------------------
// Device Population
// -------------------------------------------------------------
function populateDevices(inputDevs, outputDevs, cfg) {
  // Input devices
  micSelect.innerHTML = '';
  const hasSavedMic = cfg.selected_mic && inputDevs.some((d) => d.name === cfg.selected_mic);
  const defaultMic = inputDevs.find((d) => d.is_default) || inputDevs[0];
  const targetMic = hasSavedMic ? cfg.selected_mic : (defaultMic ? defaultMic.name : '');

  inputDevs.forEach((dev) => {
    const opt = document.createElement('option');
    opt.value = dev.name;
    opt.textContent = dev.name + (dev.is_default ? ' (Default)' : '');
    if (dev.name === targetMic) opt.selected = true;
    micSelect.appendChild(opt);
  });
  micAutoBadge.textContent = hasSavedMic ? 'Saved' : 'Auto Detected';

  // Output devices (filter out virtual cables if physical speakers exist)
  speakerSelect.innerHTML = '';
  const physicalDevs = outputDevs.filter((d) => {
    const lower = (d.name || '').toLowerCase();
    return !lower.includes('cable') && !lower.includes('virtual');
  });
  const spkList = physicalDevs.length > 0 ? physicalDevs : outputDevs;
  const hasSavedSpk = cfg.selected_speaker && spkList.some((d) => d.name === cfg.selected_speaker);
  const defaultSpk = spkList.find((d) => d.is_default) || spkList[0];
  const targetSpk = hasSavedSpk ? cfg.selected_speaker : (defaultSpk ? defaultSpk.name : '');

  spkList.forEach((dev) => {
    const opt = document.createElement('option');
    opt.value = dev.name;
    opt.textContent = dev.name + (dev.is_default ? ' (Default)' : '');
    if (dev.name === targetSpk) opt.selected = true;
    speakerSelect.appendChild(opt);
  });
  spkAutoBadge.textContent = hasSavedSpk ? 'Saved' : 'Auto Detected';
}

// -------------------------------------------------------------
// Config State Hydration
// -------------------------------------------------------------
function applyConfig(cfg) {
  isBypassed = !!cfg.bypass;
  updateMasterPowerUI(isBypassed);

  // Volumes
  const micPct = Math.round((cfg.mic_volume !== undefined ? cfg.mic_volume : 1.0) * 100);
  micVolumeSlider.value = micPct;
  micVolText.textContent = `${micPct}%`;

  const spkPct = Math.round((cfg.speaker_volume !== undefined ? cfg.speaker_volume : 1.0) * 100);
  speakerVolumeSlider.value = spkPct;
  speakerVolText.textContent = `${spkPct}%`;

  // Strength
  const strengthPct = Math.round((cfg.strength !== undefined ? cfg.strength : 1.0) * 100);
  strengthSlider.value = strengthPct;
  strengthValText.textContent = `${strengthPct}%`;

  // Mode
  let mode = cfg.mode || 'Krisp';
  if (isBypassed) mode = 'None';
  else if (mode === 'Krisp') mode = 'MaxQuality';
  updateNoiseModeUI(mode);

  // Auto-Sensitivity
  const isAutoSens = cfg.auto_sensitivity !== false;
  if (autoSensitivityToggle) autoSensitivityToggle.checked = isAutoSens;
  if (manualSensitivitySection) {
    manualSensitivitySection.classList.toggle('disabled', isAutoSens);
    manualSensitivitySection.style.display = isAutoSens ? 'none' : 'block';
  }
  if (sensitivitySlider) sensitivitySlider.disabled = isAutoSens;
  const thresh = Math.round(cfg.sensitivity_threshold !== undefined ? cfg.sensitivity_threshold : -50);
  if (sensitivitySlider) sensitivitySlider.value = thresh;
  if (sensitivityText) sensitivityText.textContent = `${thresh} dB`;

  // AEC
  if (aecToggle) aecToggle.checked = !!cfg.aec_enabled;
  if (aecDetails) aecDetails.style.display = cfg.aec_enabled ? 'flex' : 'none';

  // Push to Talk
  const isPtt = !!cfg.push_to_talk;
  if (pttToggle) pttToggle.checked = isPtt;
  if (pttControls) {
    pttControls.classList.toggle('disabled', !isPtt);
    pttControls.style.display = isPtt ? 'block' : 'none';
  }
  currentPttKey = cfg.ptt_key || 'Space';
  if (pttKeyBtn) pttKeyBtn.textContent = currentPttKey;
  updatePttStatus(false);

  // Preferences
  if (closeToTrayToggle) {
    closeToTrayToggle.checked = cfg.close_to_tray !== false;
  }
  if (startWithWindowsToggle) {
    startWithWindowsToggle.checked = !!cfg.start_with_windows;
  }
}

// -------------------------------------------------------------
// Master Power / Bypass
// -------------------------------------------------------------
function updateMasterPowerUI(bypassed) {
  isBypassed = bypassed;
  if (bypassed) {
    masterPowerBtn.classList.remove('active');
    masterPowerBtn.classList.add('bypassed');
    masterPowerText.textContent = 'Bypassed';

    engineStatusPill.className = 'status-pill bypassed';
    engineStatusText.textContent = 'Bypassed';
    if (noiseReductionBadge) {
      noiseReductionBadge.className = 'auto-badge';
      noiseReductionBadge.textContent = 'Bypassed';
    }
  } else {
    masterPowerBtn.classList.remove('bypassed');
    masterPowerBtn.classList.add('active');
    masterPowerText.textContent = 'Active';

    engineStatusPill.className = 'status-pill active';
    engineStatusText.textContent = 'Active';
    if (noiseReductionBadge) {
      noiseReductionBadge.className = 'auto-badge badge-active';
      noiseReductionBadge.textContent = 'Active';
    }
  }
}

async function toggleMasterPower() {
  const newBypass = !isBypassed;
  updateMasterPowerUI(newBypass);
  try {
    await invoke('set_bypass', { bypass: newBypass });
  } catch (err) {
    console.error('Failed to toggle bypass:', err);
  }
}

// -------------------------------------------------------------
// Mode Segmented Control
// -------------------------------------------------------------
function updateNoiseModeUI(mode) {
  currentMode = mode;
  segButtons.forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.mode === mode);
  });

  if (mode === 'None') {
    updateMasterPowerUI(true);
  } else {
    updateMasterPowerUI(false);
    if (engineStatusText) {
      engineStatusText.textContent = 'Active';
    }
  }
}

async function selectNoiseMode(mode) {
  updateNoiseModeUI(mode);
  try {
    await invoke('set_noise_mode', { mode });
  } catch (err) {
    console.error('Failed to set noise mode:', err);
  }
}

// -------------------------------------------------------------
// Live Voice Tester Suite (Hero Feature)
// -------------------------------------------------------------
function updateLiveTestUI(active) {
  isLiveMonitoring = !!active;
  if (isLiveMonitoring) {
    liveVoiceBtn.classList.add('active');
    liveVoiceIconOff.style.display = 'none';
    liveVoiceIconOn.style.display = 'block';
    liveVoiceTitle.innerHTML = '<span class="live-pulse-dot"></span>Live Monitor: ON';
    liveVoiceDesc.textContent = 'Streaming live monitor audio to headphones.';
  } else {
    liveVoiceBtn.classList.remove('active');
    liveVoiceIconOff.style.display = 'block';
    liveVoiceIconOn.style.display = 'none';
    liveVoiceTitle.textContent = 'Test Your Voice Live';
    liveVoiceDesc.textContent = 'Click icon to hear your voice live in headphones';
  }
}

async function toggleLiveTest() {
  try {
    const spkName = speakerSelect.value || null;
    const active = await invoke('toggle_live_test', { deviceName: spkName });
    updateLiveTestUI(active);
  } catch (err) {
    console.error('Failed to toggle live test:', err);
  }
}

// -------------------------------------------------------------
// Metrics & VU Meter Polling
// -------------------------------------------------------------
async function pollMetrics() {
  try {
    const metrics = await invoke('get_metrics');
    if (!metrics) return;

    // Periodic live monitor synchronization
    if (liveStatusPollCount++ % 10 === 0) {
      const liveStatus = await invoke('get_live_test_status').catch(() => null);
      if (liveStatus && liveStatus.is_active !== isLiveMonitoring) {
        updateLiveTestUI(liveStatus.is_active);
      }
    }

    // Peak VU Meter
    const inDb = metrics.input_peak_db;
    const clamped = Math.max(-60, Math.min(0, inDb));
    const pct = Math.round(((clamped + 60) / 60) * 100);
    micMeterFill.style.width = `${pct}%`;
    micMeterDb.textContent = inDb <= -60 ? '-60 dB' : `${inDb.toFixed(1)} dB`;

    // Clipping LED Badge
    if (inDb >= -0.5 || metrics.clipping_count > lastClippingCount) {
      clippingIndicator.style.display = 'inline-block';
      clearTimeout(clipTimer);
      clipTimer = setTimeout(() => {
        clippingIndicator.style.display = 'none';
      }, 700);
    }
    lastClippingCount = metrics.clipping_count || 0;

    // Noise Reduction Badge
    if (!isBypassed && noiseReductionBadge) {
      noiseReductionBadge.textContent = 'Active';
    }

    // DSP Telemetry (Safe if hidden)
    if (framesProcessedText) {
      framesProcessedText.textContent = (metrics.frames_processed || 0).toLocaleString();
    }
    if (framesPassthroughText) {
      framesPassthroughText.textContent = (metrics.frames_passthrough || 0).toLocaleString();
    }
    if (dspLatencyText) {
      dspLatencyText.textContent = `${metrics.dsp_latency_us || 0} µs`;
    }

    // Footer
    if (metrics.cpu_usage_pct !== undefined) {
      footerCpuText.textContent = `CPU ${metrics.cpu_usage_pct.toFixed(1)}%`;
    }

    // AEC Telemetry
    if (aecToggle.checked) {
      if (metrics.aec_endpoint && aecEndpointBadge) {
        aecEndpointBadge.style.display = 'inline-block';
        aecEndpointBadge.textContent = metrics.aec_endpoint;
      }
      if (aecRefDeviceText && metrics.aec_ref_device) {
        aecRefDeviceText.textContent = metrics.aec_ref_device;
      }
      if (aecRefLevelText) {
        const refPeak = metrics.aec_ref_peak_db !== undefined ? metrics.aec_ref_peak_db.toFixed(1) : '-100.0';
        aecRefLevelText.textContent = `${refPeak} dB`;
        aecRefLevelText.style.color = metrics.aec_ref_peak_db > -50 ? 'var(--accent-emerald)' : 'var(--text-muted)';
      }
      if (aecStatusText) {
        const status = metrics.aec_bypass_reason || 'Active';
        aecStatusText.textContent = status;
        if (status === 'Active') {
          aecStatusText.style.color = 'var(--accent-emerald)';
        } else if (status.includes('Silent') || status.includes('Correlation')) {
          aecStatusText.style.color = 'var(--accent-amber)';
        } else {
          aecStatusText.style.color = 'var(--text-muted)';
        }
      }
      if (aecErleText) {
        const erle = metrics.aec_erle_db !== undefined ? metrics.aec_erle_db.toFixed(1) : '0.0';
        aecErleText.textContent = `${erle} dB`;
      }
      if (aecDelayText) {
        const delay = metrics.aec_delay_ms !== undefined ? Math.round(metrics.aec_delay_ms) : 0;
        aecDelayText.textContent = `${delay} ms`;
      }
      if (aecFramesText) {
        aecFramesText.textContent = (metrics.aec_frames_processed || 0).toLocaleString();
      }
      if (metrics.loop_warning && loopWarningPill) {
        loopWarningPill.style.display = 'inline-block';
      } else if (loopWarningPill) {
        loopWarningPill.style.display = 'none';
      }
    }
  } catch (err) {
    // Fail silently on polling error to avoid spamming console
  }
}

function startMetricsPolling(intervalMs = 66) {
  if (pollIntervalId) clearInterval(pollIntervalId);
  pollIntervalId = setInterval(pollMetrics, intervalMs);
}

// -------------------------------------------------------------
// 0% Idle CPU Throttling when Window is Minimized / Hidden
// -------------------------------------------------------------
function setupCpuThrottling() {
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
      // Window is minimized or hidden in tray: completely halt polling for strictly 0% CPU
      if (pollIntervalId) {
        clearInterval(pollIntervalId);
        pollIntervalId = null;
      }
    } else {
      // Window is visible: restore 15 fps polling immediately
      startMetricsPolling(66);
      pollMetrics();
    }
  });
}

// -------------------------------------------------------------
// Push to Talk Key Handling
// -------------------------------------------------------------
function updatePttStatus(transmitting) {
  isPttTransmitting = transmitting;
  if (!pttToggle.checked) {
    pttStatusBadge.textContent = 'Disabled';
    pttStatusBadge.className = 'ptt-status';
    return;
  }
  if (transmitting) {
    pttStatusBadge.textContent = 'Transmitting';
    pttStatusBadge.className = 'ptt-status transmitting';
  } else {
    pttStatusBadge.textContent = 'Muted';
    pttStatusBadge.className = 'ptt-status';
  }
}

function isKeyMatchingPtt(e) {
  const normKey = currentPttKey.toLowerCase();
  if (normKey === 'space' && (e.code === 'Space' || e.key === ' ')) return true;
  if (e.code.toLowerCase() === normKey) return true;
  if (e.key.toLowerCase() === normKey) return true;
  return false;
}

// -------------------------------------------------------------
// Event Listeners Setup
// -------------------------------------------------------------
function setupEventListeners() {
  // Tab Navigation
  const tabButtons = document.querySelectorAll('.tab-btn');
  const tabPanes = document.querySelectorAll('.tab-pane');
  tabButtons.forEach((btn) => {
    btn.addEventListener('click', () => {
      const targetPaneId = btn.getAttribute('data-tab');
      if (!targetPaneId) return;

      tabButtons.forEach((b) => {
        b.classList.remove('active');
        b.setAttribute('aria-selected', 'false');
      });
      tabPanes.forEach((p) => p.classList.add('hidden'));

      btn.classList.add('active');
      btn.setAttribute('aria-selected', 'true');
      const targetPane = document.getElementById(targetPaneId);
      if (targetPane) {
        targetPane.classList.remove('hidden');
      }
    });
  });

  // Reduce Effects Toggle
  if (reduceEffectsToggle) {
    reduceEffectsToggle.addEventListener('change', () => {
      userManuallySetReduceEffects = true;
      setReduceEffects(reduceEffectsToggle.checked);
    });
  }

  // Master Power
  masterPowerBtn.addEventListener('click', toggleMasterPower);

  // Theme Toggle & Switcher
  themeToggleBtn.addEventListener('click', toggleTheme);
  if (themeSelector) {
    themeSelector.addEventListener('change', (e) => setThemeAndSave(e.target.value));
  }
  if (prefsThemeSelector) {
    prefsThemeSelector.addEventListener('change', (e) => setThemeAndSave(e.target.value));
  }

  // Mode Segmented Control
  segButtons.forEach((btn) => {
    btn.addEventListener('click', () => {
      selectNoiseMode(btn.dataset.mode);
    });
  });

  // Strength Slider
  strengthSlider.addEventListener('input', () => {
    const val = parseInt(strengthSlider.value, 10);
    strengthValText.textContent = `${val}%`;
  });
  strengthSlider.addEventListener('change', () => {
    const strength = parseInt(strengthSlider.value, 10) / 100.0;
    invoke('set_strength', { strength }).catch((e) => console.error('Failed to set strength:', e));
  });

  // Mic Volume
  micVolumeSlider.addEventListener('input', () => {
    const val = parseInt(micVolumeSlider.value, 10);
    micVolText.textContent = `${val}%`;
  });
  micVolumeSlider.addEventListener('change', () => {
    const volume = parseInt(micVolumeSlider.value, 10) / 100.0;
    invoke('set_mic_volume', { volume }).catch((e) => console.error('Failed to set mic volume:', e));
  });

  // Speaker Volume
  speakerVolumeSlider.addEventListener('input', () => {
    const val = parseInt(speakerVolumeSlider.value, 10);
    speakerVolText.textContent = `${val}%`;
  });
  speakerVolumeSlider.addEventListener('change', () => {
    const volume = parseInt(speakerVolumeSlider.value, 10) / 100.0;
    invoke('set_speaker_volume', { volume }).catch((e) => console.error('Failed to set speaker volume:', e));
  });

  // Device Selects
  micSelect.addEventListener('change', () => {
    invoke('set_input_device', { name: micSelect.value }).catch((e) => console.error('Failed to set mic device:', e));
    micAutoBadge.textContent = 'Selected';
  });

  speakerSelect.addEventListener('change', () => {
    invoke('set_output_device', { name: speakerSelect.value }).catch((e) => console.error('Failed to set spk device:', e));
    spkAutoBadge.textContent = 'Selected';
  });

  // Live Voice Button (Strictly Mouse Click Only - Immune to Keyboard Space / Enter / Any Key)
  liveVoiceBtn.addEventListener('click', (e) => {
    // Immediately blur so liveVoiceBtn NEVER retains document focus
    liveVoiceBtn.blur();

    // Guard against synthetic keyboard clicks (detail === 0 with no mouse pointer coordinates)
    const isSynthetic = (e && e.detail === 0 && (!e.pointerType || e.pointerType === ''));
    if (isSynthetic) {
      e.preventDefault();
      e.stopPropagation();
      return;
    }

    toggleLiveTest();
  });

  // Explicitly prevent any keyboard keydown/keyup/focus from activating liveVoiceBtn
  liveVoiceBtn.addEventListener('keydown', (e) => {
    e.preventDefault();
    e.stopPropagation();
    liveVoiceBtn.blur();
    return false;
  });
  liveVoiceBtn.addEventListener('keyup', (e) => {
    e.preventDefault();
    e.stopPropagation();
    liveVoiceBtn.blur();
    return false;
  });
  liveVoiceBtn.addEventListener('focus', () => {
    liveVoiceBtn.blur();
  });

  // A/B Monitor Switches
  if (abCleanedBtn && abOriginalBtn) {
    abCleanedBtn.addEventListener('click', () => {
      abCleanedBtn.classList.add('active');
      abOriginalBtn.classList.remove('active');
      invoke('set_live_test_ab', { mode: 'Cleaned' }).catch(() => {});
    });

    abOriginalBtn.addEventListener('click', () => {
      abOriginalBtn.classList.add('active');
      abCleanedBtn.classList.remove('active');
      invoke('set_live_test_ab', { mode: 'Original' }).catch(() => {});
    });
  }

  // Auto-Sensitivity
  if (autoSensitivityToggle) {
    autoSensitivityToggle.addEventListener('change', () => {
      const isAuto = autoSensitivityToggle.checked;
      if (manualSensitivitySection) {
        manualSensitivitySection.classList.toggle('disabled', isAuto);
        manualSensitivitySection.style.display = isAuto ? 'none' : 'block';
      }
      if (sensitivitySlider) {
        sensitivitySlider.disabled = isAuto;
      }
      invoke('set_auto_sensitivity', { enabled: isAuto }).catch((e) => console.error('Failed to set auto-sens:', e));
    });
  }

  if (sensitivitySlider) {
    sensitivitySlider.addEventListener('input', () => {
      if (sensitivityText) sensitivityText.textContent = `${sensitivitySlider.value} dB`;
    });
    sensitivitySlider.addEventListener('change', () => {
      const threshold = parseFloat(sensitivitySlider.value);
      invoke('set_sensitivity_threshold', { threshold }).catch((e) => console.error('Failed to set threshold:', e));
    });
  }

  // Echo Cancellation (AEC)
  if (aecToggle) {
    aecToggle.addEventListener('change', () => {
      const en = aecToggle.checked;
      if (aecDetails) aecDetails.style.display = en ? 'flex' : 'none';
      invoke('set_aec', { enabled: en }).catch((e) => console.error('Failed to set aec:', e));
    });
  }

  if (aecTestBtn) {
    aecTestBtn.addEventListener('click', async () => {
      aecTestBtn.disabled = true;
      aecTestBtn.textContent = 'Testing...';
      try {
        const res = await invoke('run_aec_speaker_test_command');
        const msg = res.summary || (res.passed ? `PASS: ERLE ${res.erle_db.toFixed(1)} dB` : `FAIL: ERLE ${res.erle_db.toFixed(1)} dB`);
        alert(`AEC Calibration & Self-Test:\n${msg}`);
      } catch (e) {
        alert(`Speaker Test Failed: ${e}`);
      } finally {
        aecTestBtn.disabled = false;
        aecTestBtn.textContent = 'Speaker Test';
      }
    });
  }

  // Echo Doctor Modal
  if (echoDoctorBtn) {
    echoDoctorBtn.addEventListener('click', async () => {
      if (echoDoctorModal) echoDoctorModal.style.display = 'flex';
      if (doctorSeverityBanner) doctorSeverityBanner.className = 'doctor-banner banner-clean';
      if (doctorSeverityIcon) doctorSeverityIcon.textContent = '⏳';
      if (doctorSeverityText) doctorSeverityText.textContent = 'Auditing audio routing...';
      if (doctorExplanation) doctorExplanation.textContent = 'Inspecting Windows audio endpoints, virtual cable loops, and communication routes...';
      if (doctorFixSection) doctorFixSection.style.display = 'none';

      try {
        const report = await invoke('run_echo_doctor_command');
        const sev = report.loop_severity || 'CLEAN';

        if (doctorSeverityBanner && doctorSeverityIcon && doctorSeverityText) {
          if (sev === 'FEEDBACK_LOOP' || sev === 'Loop') {
            doctorSeverityBanner.className = 'doctor-banner banner-loop';
            doctorSeverityIcon.textContent = '🚨';
            doctorSeverityText.textContent = 'Feedback Loop Detected!';
          } else if (sev === 'WARNING' || sev === 'Warning') {
            doctorSeverityBanner.className = 'doctor-banner banner-warning';
            doctorSeverityIcon.textContent = '⚠️';
            doctorSeverityText.textContent = 'Routing Loop Warning';
          } else {
            doctorSeverityBanner.className = 'doctor-banner banner-clean';
            doctorSeverityIcon.textContent = '✅';
            doctorSeverityText.textContent = 'Clean Audio Routing';
          }
        }

        if (doctorExplanation) doctorExplanation.textContent = report.plain_explanation || 'Audio routing is optimal.';

        if (doctorFixSection && doctorFixList && report.recommended_actions && report.recommended_actions.length > 0) {
          doctorFixSection.style.display = 'block';
          doctorFixList.innerHTML = '';
          report.recommended_actions.forEach((fix) => {
            const li = document.createElement('li');
            li.textContent = fix;
            doctorFixList.appendChild(li);
          });
        }

        if (doctorDiscordTips && report.discord_tips && report.discord_tips.length > 0) {
          doctorDiscordTips.innerHTML = '';
          report.discord_tips.forEach((tip) => {
            const li = document.createElement('li');
            li.textContent = tip;
            doctorDiscordTips.appendChild(li);
          });
        }
      } catch (e) {
        if (doctorExplanation) doctorExplanation.textContent = `Diagnostic audit error: ${e}`;
      }
    });
  }

  const closeEchoDoctor = () => {
    if (echoDoctorModal) echoDoctorModal.style.display = 'none';
  };
  if (closeEchoDoctorBtn) closeEchoDoctorBtn.addEventListener('click', closeEchoDoctor);
  if (dismissEchoDoctorBtn) dismissEchoDoctorBtn.addEventListener('click', closeEchoDoctor);

  // Pipeline Self-Test Modal
  if (pipelineSelfTestBtn) {
    pipelineSelfTestBtn.addEventListener('click', async () => {
      if (pipelineModal) pipelineModal.style.display = 'flex';
      if (pipelineResultBanner) pipelineResultBanner.className = 'doctor-banner banner-clean';
      if (pipelineResultIcon) pipelineResultIcon.textContent = '⏳';
      if (pipelineResultText) pipelineResultText.textContent = 'Running Pipeline Self-Test...';
      if (pipelineDetails) pipelineDetails.textContent = 'Testing DSP signal flow with synthetic audio...';

      try {
        const res = await invoke('run_pipeline_self_test_command');
        if (pipelineResultBanner && pipelineResultIcon && pipelineResultText) {
          if (res && res.success) {
            pipelineResultBanner.className = 'doctor-banner banner-clean';
            pipelineResultIcon.textContent = '✅';
            pipelineResultText.textContent = 'Pipeline Verified & Healthy';
          } else {
            pipelineResultBanner.className = 'doctor-banner banner-loop';
            pipelineResultIcon.textContent = '❌';
            pipelineResultText.textContent = 'Pipeline Diagnostic Warning';
          }
        }
        if (pipelineDetails) pipelineDetails.textContent = JSON.stringify(res, null, 2);
      } catch (e) {
        if (pipelineResultBanner && pipelineResultIcon && pipelineResultText) {
          pipelineResultBanner.className = 'doctor-banner banner-loop';
          pipelineResultIcon.textContent = '❌';
          pipelineResultText.textContent = 'Self-Test Failed';
        }
        if (pipelineDetails) pipelineDetails.textContent = `Error: ${e}`;
      }
    });
  }

  const closePipelineModal = () => {
    if (pipelineModal) pipelineModal.style.display = 'none';
  };
  if (closePipelineModalBtn) closePipelineModalBtn.addEventListener('click', closePipelineModal);
  if (dismissPipelineModalBtn) dismissPipelineModalBtn.addEventListener('click', closePipelineModal);

  // Push to Talk
  if (pttToggle) {
    pttToggle.addEventListener('change', () => {
      const en = pttToggle.checked;
      if (pttControls) {
        pttControls.classList.toggle('disabled', !en);
        pttControls.style.display = en ? 'block' : 'none';
      }
      invoke('set_push_to_talk', { enabled: en }).catch((e) => console.error('Failed to set PTT:', e));
      updatePttStatus(false);
    });
  }

  if (pttKeyBtn) {
    pttKeyBtn.addEventListener('click', () => {
      if (isRecordingKey) return;
      isRecordingKey = true;
      pttKeyBtn.textContent = 'Press any key...';
      pttKeyBtn.classList.add('recording');

      const captureKey = (e) => {
        e.preventDefault();
        e.stopPropagation();
        let key = e.code === 'Space' ? 'Space' : e.key;
        if (key.length === 1) key = key.toUpperCase();
        currentPttKey = key;
        pttKeyBtn.textContent = key;
        pttKeyBtn.classList.remove('recording');
        isRecordingKey = false;
        window.removeEventListener('keydown', captureKey, true);
        invoke('set_ptt_key', { key }).catch((err) => console.error('Failed to set ptt key:', err));
      };

      window.addEventListener('keydown', captureKey, true);
    });
  }

  // Guard against any keyboard key (Space, Enter, etc.) triggering liveVoiceBtn
  window.addEventListener('keydown', (e) => {
    if (document.activeElement === liveVoiceBtn || e.target === liveVoiceBtn) {
      liveVoiceBtn.blur();
      e.preventDefault();
      e.stopPropagation();
    }
  }, true);

  // Global PTT Key Listeners
  window.addEventListener('keydown', (e) => {
    if (isRecordingKey || !pttToggle.checked) return;
    if (isKeyMatchingPtt(e) && !isPttTransmitting) {
      updatePttStatus(true);
      invoke('set_ptt_state', { active: true }).catch(() => {});
    }
  });

  window.addEventListener('keyup', (e) => {
    if (isRecordingKey || !pttToggle.checked) return;
    if (isKeyMatchingPtt(e) && isPttTransmitting) {
      updatePttStatus(false);
      invoke('set_ptt_state', { active: false }).catch(() => {});
    }
  });

  // Preferences Listeners
  if (closeToTrayToggle) {
    closeToTrayToggle.addEventListener('change', () => {
      const en = closeToTrayToggle.checked;
      invoke('set_close_to_tray', { enabled: en }).catch((e) => console.error('Failed to set close_to_tray:', e));
    });
  }

  if (startWithWindowsToggle) {
    startWithWindowsToggle.addEventListener('change', async () => {
      const en = startWithWindowsToggle.checked;
      try {
        await invoke('set_start_with_windows', { enabled: en });
        const autostart = await invoke('get_autostart_status');
        if (autostart && startupWarning) {
          startupWarning.style.display = autostart.disabled_by_os ? 'inline-block' : 'none';
        }
      } catch (e) {
        console.error('Failed to set start_with_windows:', e);
      }
    });
  }

  if (openStartupSettingsBtn) {
    openStartupSettingsBtn.addEventListener('click', () => {
      invoke('open_windows_startup_settings').catch((e) => console.error('Failed to open startup settings:', e));
    });
  }

  if (togglePinGuideBtn && pinGuideDetails) {
    togglePinGuideBtn.addEventListener('click', () => {
      const isHidden = pinGuideDetails.style.display === 'none';
      pinGuideDetails.style.display = isHidden ? 'block' : 'none';
      togglePinGuideBtn.textContent = isHidden ? 'Hide Guide' : 'Pinning Guide';
    });
  }

  if (openTaskbarSettingsBtn) {
    openTaskbarSettingsBtn.addEventListener('click', () => {
      invoke('open_windows_taskbar_settings').catch((e) => console.error('Failed to open taskbar settings:', e));
    });
  }

  if (dismissTrayToastBtn) {
    dismissTrayToastBtn.addEventListener('click', () => {
      hideTrayToastNotification();
    });
  }
}

// -------------------------------------------------------------
// Tray Toast Notification
// -------------------------------------------------------------
function showTrayToastNotification() {
  if (!trayToast) return;
  trayToast.style.display = 'flex';
  setTimeout(() => {
    trayToast.classList.add('visible');
  }, 10);
  setTimeout(() => {
    hideTrayToastNotification();
  }, 6000);
}

function hideTrayToastNotification() {
  if (!trayToast) return;
  trayToast.classList.remove('visible');
  setTimeout(() => {
    trayToast.style.display = 'none';
  }, 300);
}

// Start on DOM ready
document.addEventListener('DOMContentLoaded', init);
