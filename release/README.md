# 180hz nc — Official Release Binaries

This folder contains the official production Windows x64 executables for **180hz nc** featuring Dual Themes (Classic Dark & Soft Glass), real-time AI noise cancellation, and acoustic echo cancellation (AEC).

## Binaries

| File | Type | Size | Description | Direct Link |
| :--- | :--- | :--- | :--- | :--- |
| **`180hz-nc-setup.exe`** | All-in-One Installer | 9.9 MB | **Turnkey setup:** Installs 180hz nc, bundled VB-CABLE virtual audio driver, Start Menu & Desktop shortcuts, and uninstaller. | [Download](180hz-nc-setup.exe) |
| **`180hz-nc.exe`** | Standalone Executable | 7.9 MB | **Zero install:** Standalone portable executable with Dual Theme Switcher (Classic Dark & Soft Glass), DSP engine, and local Inter fonts. | [Download](180hz-nc.exe) |

## SHA-256 Checksums

```
7176620fd066ba3a5305fbc2932782a27bd4a3839c6a82e17e653aed3459b276  180hz-nc.exe
3dc6d0c5e3f5961fa4c93cc7027e13b81705ab0a213018d611e4708a855ee212  180hz-nc-setup.exe
```

### Verification in PowerShell

```powershell
Get-FileHash release\180hz-nc-setup.exe -Algorithm SHA256
Get-FileHash release\180hz-nc.exe -Algorithm SHA256
```
