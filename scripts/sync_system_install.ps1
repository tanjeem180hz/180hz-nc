# Copy WebView2Loader.dll and release binaries to Program Files and System32 if possible
$srcDll = "E:\GitHubProject\NoiseCancelator\release\WebView2Loader.dll"
$srcExe = "E:\GitHubProject\NoiseCancelator\release\180hz-nc.exe"
$progFilesDir = "C:\Program Files\180hz nc"

if (!(Test-Path $progFilesDir)) {
    New-Item -ItemType Directory -Path $progFilesDir -Force | Out-Null
}

try {
    Copy-Item $srcDll -Destination (Join-Path $progFilesDir "WebView2Loader.dll") -Force -ErrorAction Stop
    Copy-Item $srcExe -Destination (Join-Path $progFilesDir "180hz nc.exe") -Force -ErrorAction Stop
    Copy-Item $srcExe -Destination (Join-Path $progFilesDir "180hz-nc.exe") -Force -ErrorAction Stop
    Write-Host "Successfully updated C:\Program Files\180hz nc"
} catch {
    Write-Warning "Could not write to Program Files directly: $_"
}

try {
    Copy-Item $srcDll -Destination "C:\Windows\System32\WebView2Loader.dll" -Force -ErrorAction Stop
    Write-Host "Successfully placed WebView2Loader.dll in C:\Windows\System32"
} catch {
    Write-Warning "Could not write to System32 directly: $_"
}

# Update all user and machine shortcuts to point directly to working executable
$wsh = New-Object -ComObject WScript.Shell
$localExe = "$env:LOCALAPPDATA\180hz nc\180hz nc.exe"
$localDir = "$env:LOCALAPPDATA\180hz nc"

# Ensure local appdata has the DLL and exe
if (!(Test-Path $localDir)) {
    New-Item -ItemType Directory -Path $localDir -Force | Out-Null
}
Copy-Item $srcDll -Destination (Join-Path $localDir "WebView2Loader.dll") -Force
Copy-Item $srcExe -Destination (Join-Path $localDir "180hz nc.exe") -Force
Copy-Item $srcExe -Destination (Join-Path $localDir "180hz-nc.exe") -Force

# Update User Start Menu shortcut
$userStartLnk = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\180hz nc.lnk"
$sc1 = $wsh.CreateShortcut($userStartLnk)
$sc1.TargetPath = $localExe
$sc1.WorkingDirectory = $localDir
$sc1.Description = "180hz nc - Real-Time AI Noise Suppression"
$sc1.Save()

# Update Desktop shortcut
$desk = [Environment]::GetFolderPath('Desktop')
if (Test-Path $desk) {
    $sc2 = $wsh.CreateShortcut((Join-Path $desk "180hz nc.lnk"))
    $sc2.TargetPath = $localExe
    $sc2.WorkingDirectory = $localDir
    $sc2.Description = "180hz nc - Real-Time AI Noise Suppression"
    $sc2.Save()
}

Write-Host "Shortcuts and LocalAppData sync complete."
