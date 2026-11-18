$chromePath = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
if (-not (Test-Path $chromePath)) {
    $chromePath = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
}

$uiPath = (Resolve-Path "crates\quietmic-app\ui\index.html").Path
$outDir = (Resolve-Path "docs\screenshots").Path

$shots = @(
    @{ name = "theme_night_lab.png"; hash = "#theme=night-lab&tab=tab-lab" },
    @{ name = "theme_daylight.png"; hash = "#theme=daylight&tab=tab-lab" },
    @{ name = "theme_amber_console.png"; hash = "#theme=amber-console&tab=tab-lab" },
    @{ name = "tab_devices.png"; hash = "#theme=night-lab&tab=tab-devices" },
    @{ name = "tab_studio.png"; hash = "#theme=night-lab&tab=tab-studio" },
    @{ name = "tab_diagnostics.png"; hash = "#theme=night-lab&tab=tab-diagnostics" }
)

foreach ($s in $shots) {
    $outFile = Join-Path $outDir $s.name
    $url = "file:///$($uiPath.Replace('\', '/'))$($s.hash)"
    Write-Host "Capturing $($s.name)..."
    & $chromePath --headless=new --disable-gpu --hide-scrollbars --window-size=440,680 --virtual-time-budget=2000 "--screenshot=$outFile" $url
    Start-Sleep -Milliseconds 250
}

Write-Host "All screenshots captured successfully!"
