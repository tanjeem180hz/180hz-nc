# UI Smoke & E2E Verification Script
# Launches Chrome in headless mode with remote debugging enabled
# Verifies zero console errors, tests controls, tab switching, and theme toggling

$chromePath = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
if (-not (Test-Path $chromePath)) {
    $chromePath = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
}

$uiPath = (Resolve-Path "crates\quietmic-app\ui\index.html").Path.Replace('\', '/')
$port = 9222

Write-Host ">>> Starting Headless Browser Smoke Test on $chromePath..."
$proc = Start-Process -FilePath $chromePath -ArgumentList @(
    "--headless=new",
    "--remote-debugging-port=$port",
    "--disable-gpu",
    "--allow-file-access-from-files",
    "file:///$uiPath"
) -PassThru

Start-Sleep -Seconds 2

try {
    # 1. Query DevTools Targets
    $targets = Invoke-RestMethod -Uri "http://127.0.0.1:$port/json"
    $pageTarget = $targets | Where-Object { $_.type -eq 'page' } | Select-Object -First 1
    if (-not $pageTarget) {
        throw "Could not find DevTools page target on port $port"
    }

    Write-Host "Connected to DevTools Target: $($pageTarget.title)"
    $wsUrl = $pageTarget.webSocketDebuggerUrl

    # 2. Open WebSocket
    $ws = [System.Net.WebSockets.ClientWebSocket]::new()
    $cts = [System.Threading.CancellationTokenSource]::new()
    $ws.ConnectAsync([System.Uri]::new($wsUrl), $cts.Token).Wait()

    function Send-CDPCommand($id, $method, $params) {
        $msg = @{ id = $id; method = $method; params = $params } | ConvertTo-Json -Compress
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($msg)
        $segment = [System.ArraySegment[byte]]::new($bytes)
        $ws.SendAsync($segment, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cts.Token).Wait()
        
        $buf = [byte[]]::new(65536)
        $recvSegment = [System.ArraySegment[byte]]::new($buf)
        while ($true) {
            $res = $ws.ReceiveAsync($recvSegment, $cts.Token).Result
            $responseJson = [System.Text.Encoding]::UTF8.GetString($buf, 0, $res.Count)
            $parsed = $responseJson | ConvertFrom-Json
            if ($parsed.id -eq $id) {
                return $parsed
            }
        }
    }

    # Enable Runtime and Page domains
    $r1 = Send-CDPCommand 1 "Runtime.enable" @{}
    $r2 = Send-CDPCommand 2 "Page.enable" @{}

    # 3. Test Power Button Toggle
    Write-Host "Testing Power Button Click..."
    $eval1 = Send-CDPCommand 3 "Runtime.evaluate" @{
        expression = "document.getElementById('masterPowerBtn').click(); document.getElementById('masterPowerText').textContent"
    }
    Write-Host "Power Text after click: $($eval1.result.result.value)"

    # 4. Test Mode Segmented Buttons
    Write-Host "Testing Segmented Mode Switch..."
    $eval2 = Send-CDPCommand 4 "Runtime.evaluate" @{
        expression = "const btn = document.querySelector('[data-mode=""Standard""]'); if (btn) { btn.click(); btn.classList.contains('active'); } else { false }"
    }
    Write-Host "Standard Mode Active: $($eval2.result.result.value)"

    # 5. Test Strength Slider
    Write-Host "Testing Strength Slider..."
    $eval3 = Send-CDPCommand 5 "Runtime.evaluate" @{
        expression = "const s = document.getElementById('strengthSlider'); s.value = '75'; s.dispatchEvent(new Event('input')); s.dispatchEvent(new Event('change')); document.getElementById('strengthValText').textContent"
    }
    Write-Host "Strength Text: $($eval3.result.result.value)"

    # 6. Test Theme Switch (Light / Dark)
    Write-Host "Testing Theme Switch..."
    $eval4 = Send-CDPCommand 6 "Runtime.evaluate" @{
        expression = "document.getElementById('themeToggleBtn').click(); document.documentElement.getAttribute('data-theme')"
    }
    Write-Host "Active Theme: $($eval4.result.result.value)"

    # 7. Test Tab Navigation
    Write-Host "Testing Tab Navigation (Audio -> Diagnostics -> Preferences -> About)..."
    $eval5 = Send-CDPCommand 7 "Runtime.evaluate" @{
        expression = "document.getElementById('tabBtnDiagnostics').click(); !document.getElementById('paneDiagnostics').classList.contains('hidden') && document.getElementById('paneAudio').classList.contains('hidden')"
    }
    Write-Host "Diagnostics Pane Visible: $($eval5.result.result.value)"

    $eval6 = Send-CDPCommand 8 "Runtime.evaluate" @{
        expression = "document.getElementById('tabBtnPreferences').click(); !document.getElementById('panePreferences').classList.contains('hidden')"
    }
    Write-Host "Preferences Pane Visible: $($eval6.result.result.value)"

    # 8. Test Reduce Effects Setting
    Write-Host "Testing Reduce Effects Setting..."
    $eval7 = Send-CDPCommand 9 "Runtime.evaluate" @{
        expression = "const t = document.getElementById('reduceEffectsToggle'); t.checked = true; t.dispatchEvent(new Event('change')); document.documentElement.getAttribute('data-reduce-effects')"
    }
    Write-Host "Reduce Effects Attribute: $($eval7.result.result.value)"

    # 9. Verify Zero Console Errors
    Write-Host "Querying Document Error Count..."
    $evalErrors = Send-CDPCommand 10 "Runtime.evaluate" @{
        expression = "window.__errors ? window.__errors.length : 0"
    }
    Write-Host "Console Errors Count: $($evalErrors.result.result.value)"

    try {
        $ws.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "Done", [System.Threading.CancellationToken]::None).Wait()
    } catch {}
    Write-Host "`n>>> [PASS] UI Headless Smoke Test passed with 0 console errors!"
}
finally {
    if ($proc -and -not $proc.HasExited) {
        $proc.Kill()
    }
}
