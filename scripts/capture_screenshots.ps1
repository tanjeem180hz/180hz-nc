$chromePath = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
if (-not (Test-Path $chromePath)) {
    $chromePath = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
}

$uiPath = (Resolve-Path "crates\quietmic-app\ui\index.html").Path
$outDir = (Resolve-Path "docs\screenshots").Path

$shots = @(
    @{ name = "theme_night_lab.png"; query = "#theme=dark" },
    @{ name = "theme_daylight.png"; query = "#theme=light" },
    @{ name = "theme_classic_dark.png"; query = "#theme=dark" },
    @{ name = "theme_classic_light.png"; query = "#theme=light" }
)

foreach ($s in $shots) {
    $outFile = Join-Path $outDir $s.name
    $url = "file:///$($uiPath.Replace('\', '/'))$($s.query)"
    Write-Host "Capturing $($s.name)..."
    & $chromePath --headless=new --disable-gpu --hide-scrollbars --window-size=460,780 --virtual-time-budget=2000 "--screenshot=$outFile" $url
    Start-Sleep -Milliseconds 250
}

Write-Host "All screenshots captured successfully!"
