$chromePath = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
if (-not (Test-Path $chromePath)) {
    $chromePath = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
}

$uiPath = (Resolve-Path "crates\quietmic-app\ui\index.html").Path
$outDir = (Resolve-Path "docs\screenshots").Path
if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
}

$shots = @(
    # Core Soft Glass Theme Variants (100% DPI)
    @{ name = "theme_soft_glass_dark_100.png"; query = "#theme=dark"; scale = "1.0" },
    @{ name = "theme_soft_glass_light_100.png"; query = "#theme=light"; scale = "1.0" },

    # Core Soft Glass Theme Variants (150% DPI)
    @{ name = "theme_soft_glass_dark_150.png"; query = "#theme=dark"; scale = "1.5" },
    @{ name = "theme_soft_glass_light_150.png"; query = "#theme=light"; scale = "1.5" },

    # Core Soft Glass Theme Variants (200% DPI)
    @{ name = "theme_soft_glass_dark_200.png"; query = "#theme=dark"; scale = "2.0" },
    @{ name = "theme_soft_glass_light_200.png"; query = "#theme=light"; scale = "2.0" },

    # Reduce Effects (Solid Tinted Surfaces for Low-End GPU)
    @{ name = "theme_reduce_effects_dark.png"; query = "#theme=dark&reduce-effects"; scale = "1.0" },
    @{ name = "theme_reduce_effects_light.png"; query = "#theme=light&reduce-effects"; scale = "1.0" },

    # Secondary Pages: Diagnostics Tab
    @{ name = "tab_diagnostics_dark.png"; query = "#theme=dark&tab-diagnostics"; scale = "1.0" },
    @{ name = "tab_diagnostics_light.png"; query = "#theme=light&tab-diagnostics"; scale = "1.0" },

    # Secondary Pages: Preferences Tab
    @{ name = "tab_preferences_dark.png"; query = "#theme=dark&tab-preferences"; scale = "1.0" },
    @{ name = "tab_preferences_light.png"; query = "#theme=light&tab-preferences"; scale = "1.0" },

    # Secondary Pages: About Tab
    @{ name = "tab_about_dark.png"; query = "#theme=dark&tab-about"; scale = "1.0" },
    @{ name = "tab_about_light.png"; query = "#theme=light&tab-about"; scale = "1.0" },

    # Modals: Echo Doctor
    @{ name = "modal_echo_doctor_dark.png"; query = "#theme=dark&echo-doctor"; scale = "1.0" },
    @{ name = "modal_echo_doctor_light.png"; query = "#theme=light&echo-doctor"; scale = "1.0" },

    # Modals: Self-Test DSP Pipeline
    @{ name = "modal_selftest_dark.png"; query = "#theme=dark&pipeline"; scale = "1.0" },
    @{ name = "modal_selftest_light.png"; query = "#theme=light&pipeline"; scale = "1.0" },

    # Toast: Tray Notification
    @{ name = "toast_tray_notification.png"; query = "#theme=dark&toast"; scale = "1.0" },

    # Backward Compatibility Aliases for README & Docs
    @{ name = "theme_night_lab.png"; query = "#theme=dark"; scale = "1.0" },
    @{ name = "theme_daylight.png"; query = "#theme=light"; scale = "1.0" },
    @{ name = "theme_classic_dark.png"; query = "#theme=dark"; scale = "1.0" },
    @{ name = "theme_classic_light.png"; query = "#theme=light"; scale = "1.0" },
    @{ name = "tab_diagnostics.png"; query = "#theme=dark&tab-diagnostics"; scale = "1.0" },
    @{ name = "echo_doctor_modal.png"; query = "#theme=dark&echo-doctor"; scale = "1.0" }
)

Write-Host "Capturing $($shots.Count) Soft Glass theme screenshots across states and DPIs..."
foreach ($s in $shots) {
    $outFile = Join-Path $outDir $s.name
    $url = "file:///$($uiPath.Replace('\', '/'))$($s.query)"
    Write-Host "Capturing $($s.name) at DPI scale $($s.scale)..."
    & $chromePath --headless=new --disable-gpu --hide-scrollbars --window-size=460,780 "--force-device-scale-factor=$($s.scale)" --virtual-time-budget=2000 "--screenshot=$outFile" $url
    Start-Sleep -Milliseconds 150
}

Write-Host "All Soft Glass screenshots captured successfully in $outDir!"
