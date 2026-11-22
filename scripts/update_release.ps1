# Stop running 180hz nc process if active so files can be updated
Get-Process -Name "180hz*" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# Remove extra old exe in target\release
if (Test-Path "target/release/180hz nc.exe.old") {
    Remove-Item "target/release/180hz nc.exe.old" -Force
}

# Update release binaries in target\release
Copy-Item "target/release/quietmic-app.exe" "target/release/180hz nc.exe" -Force
Copy-Item "target/release/quietmic-app.exe" "target/release/180hz-nc.exe" -Force
Copy-Item "target/release/quietmic-app.exe" "release/180hz-nc.exe" -Force
Copy-Item "target/release/WebView2Loader.dll" "release/WebView2Loader.dll" -Force

# Copy installer directly from target\release\180hz-nc-setup.exe
Copy-Item "target/release/180hz-nc-setup.exe" "release/180hz-nc-setup.exe" -Force

# Create portable package
$staging = "target/portable_staging"
if (Test-Path $staging) {
    Remove-Item $staging -Recurse -Force
}
New-Item -ItemType Directory -Path $staging -Force | Out-Null
Copy-Item "target/release/quietmic-app.exe" "$staging/180hz nc.exe" -Force
Copy-Item "target/release/quietmic-app.exe" "$staging/180hz-nc.exe" -Force
Copy-Item "release/WebView2Loader.dll" "$staging/WebView2Loader.dll" -Force
Copy-Item "README.md" "$staging/README.md" -Force

if (Test-Path "release/180hz-nc-portable.zip") {
    Remove-Item "release/180hz-nc-portable.zip" -Force
}
Compress-Archive -Path "$staging/*" -DestinationPath "release/180hz-nc-portable.zip" -Force
Remove-Item $staging -Recurse -Force

# Update LocalAppData
$appData = Join-Path $env:LOCALAPPDATA "180hz nc"
if (!(Test-Path $appData)) {
    New-Item -ItemType Directory -Path $appData -Force | Out-Null
}
Copy-Item "target/release/quietmic-app.exe" (Join-Path $appData "180hz nc.exe") -Force
Copy-Item "target/release/quietmic-app.exe" (Join-Path $appData "180hz-nc.exe") -Force
Copy-Item "release/WebView2Loader.dll" (Join-Path $appData "WebView2Loader.dll") -Force
Write-Host "Updated LocalAppData at $appData successfully."

# Update shortcuts and User PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$appData*") {
    $newPath = "$appData;$userPath"
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
}

$ws = New-Object -ComObject WScript.Shell
$startMenuDir = "C:\Users\pbpb4\AppData\Roaming\Microsoft\Windows\Start Menu\Programs"
$scStart = $ws.CreateShortcut("$startMenuDir\180hz nc.lnk")
$scStart.TargetPath = (Join-Path $appData "180hz nc.exe")
$scStart.WorkingDirectory = $appData
$scStart.Description = "180hz nc - Real-Time AI Noise Suppression"
$scStart.Save()

$desktopDir = [Environment]::GetFolderPath('Desktop')
if (Test-Path $desktopDir) {
    $scDesktop = $ws.CreateShortcut("$desktopDir\180hz nc.lnk")
    $scDesktop.TargetPath = (Join-Path $appData "180hz nc.exe")
    $scDesktop.WorkingDirectory = $appData
    $scDesktop.Description = "180hz nc - Real-Time AI Noise Suppression"
    $scDesktop.Save()
}

# Update SHA256SUMS.txt
$files = @("180hz-nc-setup.exe", "180hz-nc.exe", "180hz-nc-portable.zip", "WebView2Loader.dll")
$lines = @()
foreach ($f in $files) {
    $full = Join-Path "release" $f
    if (Test-Path $full) {
        $hash = (Get-FileHash -Path $full -Algorithm SHA256).Hash.ToLower()
        $lines += "$hash  $f"
    }
}
$lines | Out-File -FilePath "release/SHA256SUMS.txt" -Encoding ascii
Write-Host "Updated SHA256SUMS.txt successfully."
