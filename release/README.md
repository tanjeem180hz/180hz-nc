# 180hz nc — Official Release Binaries

This folder contains the official production Windows x64 executables for **180hz nc** featuring the Soft Glass UI theme, real-time AI noise cancellation, and acoustic echo cancellation (AEC).

## Binaries

| File | Type | Size | Description | Direct Link |
| :--- | :--- | :--- | :--- | :--- |
| **`180hz-nc-setup.exe`** | All-in-One Installer | 9.9 MB | **Turnkey setup:** Installs 180hz nc, bundled VB-CABLE virtual audio driver, Start Menu & Desktop shortcuts, and uninstaller. | [Download](180hz-nc-setup.exe) |
| **`180hz-nc.exe`** | Standalone Executable | 7.9 MB | **Zero install:** Standalone portable executable with embedded Soft Glass UI, DSP engine, and local Inter fonts. | [Download](180hz-nc.exe) |

## SHA-256 Checksums

```
c07c0a5a98d003a12c787fef2ac957818fd9bb29748636d8cd15ca20ec314d8c  180hz-nc.exe
4702861781905c9f2dc2d13f68f680ac43d4ac5873cd209144981883b15f8c09  180hz-nc-setup.exe
```

### Verification in PowerShell

```powershell
Get-FileHash release\180hz-nc-setup.exe -Algorithm SHA256
Get-FileHash release\180hz-nc.exe -Algorithm SHA256
```
