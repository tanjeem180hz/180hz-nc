# 180hz nc — Official Release Binaries

This folder contains the official production Windows x64 executables for **180hz nc** featuring Dual Themes (Classic Dark & Soft Glass), real-time AI noise cancellation, and acoustic echo cancellation (AEC).

## Binaries

| File | Type | Size | Description | Direct Link |
| :--- | :--- | :--- | :--- | :--- |
| **`180hz-nc-setup.exe`** | All-in-One Installer | ~10.1 MB | **Turnkey setup (Recommended):** Installs 180hz nc, bundled WebView2Loader.dll, bundled VB-CABLE virtual audio driver, Start Menu & Desktop shortcuts, and uninstaller. | [Download](180hz-nc-setup.exe) |
| **`180hz-nc-portable.zip`** | Portable ZIP Archive | ~3.6 MB | **Zero install:** Contains `180hz-nc.exe` and `WebView2Loader.dll`. Extract and run from any folder or USB drive. | [Download](180hz-nc-portable.zip) |
| **`180hz-nc.exe`** | Standalone Executable | ~7.9 MB | Standalone portable executable. Requires `WebView2Loader.dll` in the same directory. | [Download](180hz-nc.exe) |
| **`WebView2Loader.dll`** | Microsoft WebView2 Loader | 157 KB | Required companion DLL for standalone `180hz-nc.exe`. | [Download](WebView2Loader.dll) |

## SHA-256 Checksums

```
ebe9cfcad68a0c4b83ce3fe3120e6bfff27d1df70496df7403b8915ca07d480f  180hz-nc-setup.exe
7176620fd066ba3a5305fbc2932782a27bd4a3839c6a82e17e653aed3459b276  180hz-nc.exe
86545b66cdb0603bc26b626fb9ad610cb6e71f28d468f5ea66df23b03dda96d5  WebView2Loader.dll
29869d3110dd3fcb133ef3fed6805cb34b5716cba4d290efcefb0adb74f7831f  180hz-nc-portable.zip
```

### Verification in PowerShell

```powershell
Get-FileHash release\180hz-nc-setup.exe -Algorithm SHA256
Get-FileHash release\180hz-nc.exe -Algorithm SHA256
```
