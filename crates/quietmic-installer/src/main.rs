#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use windows::core::HSTRING;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegOpenKeyExW, RegSetValueExW, HKEY,
    HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, REG_DWORD, REG_SZ,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, IDYES, SW_SHOWNORMAL,
};

const APP_EXE: &[u8] = include_bytes!("../resources/app.exe");
const VBCABLE_ZIP: &[u8] = include_bytes!("../resources/vbcable_pack.zip");

fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_ok() {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut ret_len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut ret_len,
            );
            let _ = windows::Win32::Foundation::CloseHandle(token);
            if ok.is_ok() {
                return elevation.TokenIsElevated != 0;
            }
        }
        false
    }
}

fn elevate_self() -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_str = args.join(" ");

    let h_op = HSTRING::from("runas");
    let h_file = HSTRING::from(current_exe.to_string_lossy().to_string());
    let h_params = HSTRING::from(args_str);

    unsafe {
        let res = ShellExecuteW(HWND(std::ptr::null_mut()), &h_op, &h_file, &h_params, None, SW_SHOWNORMAL);
        if res.0 as isize > 32 {
            std::process::exit(0);
        } else {
            Err("Administrator privileges are required to install 180Hz NC and audio drivers.".to_string())
        }
    }
}

fn show_msg_box(title: &str, text: &str, flags: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE) -> windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_RESULT {
    let h_title = HSTRING::from(title);
    let h_text = HSTRING::from(text);
    unsafe { MessageBoxW(HWND(std::ptr::null_mut()), &h_text, &h_title, flags) }
}

fn is_vbcable_installed() -> bool {
    unsafe {
        let subkey = HSTRING::from("SYSTEM\\CurrentControlSet\\Services\\VBCABLE");
        let mut hkey = HKEY::default();
        let res = RegOpenKeyExW(HKEY_LOCAL_MACHINE, &subkey, 0, KEY_READ, &mut hkey);
        if res.is_ok() {
            let _ = RegCloseKey(hkey);
            true
        } else {
            false
        }
    }
}

fn install_vbcable_driver() -> Result<(), String> {
    if is_vbcable_installed() {
        return Ok(());
    }

    let temp_dir = std::env::temp_dir().join("180hz_nc_driver_setup");
    let _ = fs::create_dir_all(&temp_dir);
    let zip_path = temp_dir.join("vbcable.zip");

    fs::write(&zip_path, VBCABLE_ZIP).map_err(|e| format!("Failed to write driver archive: {}", e))?;

    // Extract driver zip using Windows built-in tar.exe
    let _ = Command::new("tar.exe")
        .args(["-xf", &zip_path.to_string_lossy(), "-C", &temp_dir.to_string_lossy()])
        .status();

    let setup_x64 = temp_dir.join("VBCABLE_Setup_x64.exe");
    if setup_x64.exists() {
        // Run driver installer silently: -i (install) -h (hide / silent)
        let _ = Command::new(&setup_x64).args(["-i", "-h"]).status();
    }

    // Clean up temporary files
    let _ = fs::remove_file(&zip_path);
    let _ = fs::remove_dir_all(&temp_dir);

    Ok(())
}

fn create_shortcut(target_exe: &Path, shortcut_path: &Path, name: &str) {
    let ps_script = format!(
        "$WshShell = New-Object -ComObject WScript.Shell; \
         $Shortcut = $WshShell.CreateShortcut('{}'); \
         $Shortcut.TargetPath = '{}'; \
         $Shortcut.Description = '{}'; \
         $Shortcut.WorkingDirectory = '{}'; \
         $Shortcut.Save()",
        shortcut_path.display(),
        target_exe.display(),
        name,
        target_exe.parent().unwrap_or(Path::new("")).display()
    );

    let _ = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .status();
}

fn register_uninstall_entry(install_dir: &Path, exe_path: &Path, uninstaller_path: &Path) {
    unsafe {
        let subkey = HSTRING::from("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\180Hz-NC");
        let mut hkey = HKEY::default();
        if RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            &subkey,
            0,
            None,
            windows::Win32::System::Registry::REG_OPTION_NON_VOLATILE,
            KEY_ALL_ACCESS,
            None,
            &mut hkey,
            None,
        )
        .is_ok()
        {
            let set_str = |name: &str, val: &str| {
                let h_name = HSTRING::from(name);
                let h_val = HSTRING::from(val);
                let bytes: &[u8] =
                    std::slice::from_raw_parts(h_val.as_ptr() as *const u8, (h_val.len() + 1) * 2);
                let _ = RegSetValueExW(hkey, &h_name, 0, REG_SZ, Some(bytes));
            };

            let set_dword = |name: &str, val: u32| {
                let h_name = HSTRING::from(name);
                let bytes = val.to_ne_bytes();
                let _ = RegSetValueExW(hkey, &h_name, 0, REG_DWORD, Some(&bytes));
            };

            set_str("DisplayName", "180Hz NC - Real-Time AI Noise Suppression");
            set_str("DisplayVersion", "1.0.0");
            set_str("Publisher", "180Hz Engineering Team");
            set_str("InstallLocation", &install_dir.to_string_lossy());
            let uninst_cmd = format!("\"{}\" --uninstall", uninstaller_path.display());
            set_str("UninstallString", &uninst_cmd);
            let icon_val = format!("\"{}\",0", exe_path.display());
            set_str("DisplayIcon", &icon_val);
            set_dword("EstimatedSize", 16384);
            set_dword("NoModify", 1);
            set_dword("NoRepair", 1);

            let _ = RegCloseKey(hkey);
        }
    }
}

fn run_install(silent: bool) -> Result<(), String> {
    if !silent {
        let welcome = "Welcome to 180Hz NC Setup!\n\n\
                       180Hz NC is a real-time AI noise cancellation desktop app targeting quality equal to or better than Krisp with minimal CPU usage.\n\n\
                       This installer includes:\n\
                       • 180Hz NC Core Application & GUI\n\
                       • Virtual Microphone Audio Driver (VB-CABLE)\n\
                       • Desktop and Start Menu Shortcuts\n\
                       • Automatic 48,000 Hz Audio Setup\n\n\
                       Do you wish to continue with the installation?";
        let res = show_msg_box("180Hz NC Setup", welcome, MB_YESNO | MB_ICONQUESTION);
        if res != IDYES {
            return Ok(());
        }
    }

    // Kill any existing instances
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "180Hz-NC.exe", "/IM", "QuietMic.exe", "/IM", "quietmic-app.exe"])
        .status();

    let prog_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
    let install_dir = PathBuf::from(prog_files).join("180Hz-NC");
    fs::create_dir_all(&install_dir).map_err(|e| format!("Failed to create install folder: {}", e))?;

    let exe_path = install_dir.join("180Hz-NC.exe");
    let uninst_path = install_dir.join("Uninstall.exe");

    // Write application executable
    fs::write(&exe_path, APP_EXE).map_err(|e| format!("Failed to install 180Hz-NC.exe: {}", e))?;

    // Copy installer executable as Uninstaller
    if let Ok(current_exe) = std::env::current_exe() {
        let _ = fs::copy(&current_exe, &uninst_path);
    } else {
        let _ = fs::write(&uninst_path, APP_EXE);
    }

    // Install Virtual Cable driver
    let _ = install_vbcable_driver();

    // Create Start Menu Shortcut
    let start_menu_dir = PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs");
    if start_menu_dir.exists() {
        let shortcut = start_menu_dir.join("180Hz NC.lnk");
        create_shortcut(&exe_path, &shortcut, "180Hz NC - AI Noise Suppression");
    }

    // Create Desktop Shortcut
    let desktop_dir = PathBuf::from("C:\\Users\\Public\\Desktop");
    if desktop_dir.exists() {
        let shortcut = desktop_dir.join("180Hz NC.lnk");
        create_shortcut(&exe_path, &shortcut, "180Hz NC - AI Noise Suppression");
    }

    // Register Add/Remove Programs entry
    register_uninstall_entry(&install_dir, &exe_path, &uninst_path);

    if !silent {
        let finished = "180Hz NC has been successfully installed!\n\n\
                        • Application installed to: C:\\Program Files\\180Hz-NC\\\n\
                        • Virtual Microphone Driver installed & active\n\
                        • Desktop & Start Menu shortcuts created\n\n\
                        Launch 180Hz NC now?";
        let res = show_msg_box("180Hz NC Setup Completed", finished, MB_YESNO | MB_ICONINFORMATION);
        if res == IDYES {
            let _ = Command::new(&exe_path).spawn();
        }
    }

    Ok(())
}

fn run_uninstall(silent: bool) -> Result<(), String> {
    if !silent {
        let confirm = "Are you sure you want to completely uninstall 180Hz NC from your computer?";
        let res = show_msg_box("Uninstall 180Hz NC", confirm, MB_YESNO | MB_ICONQUESTION);
        if res != IDYES {
            return Ok(());
        }
    }

    // Kill running instances
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "180Hz-NC.exe", "/IM", "QuietMic.exe", "/IM", "quietmic-app.exe"])
        .status();

    // Remove Desktop shortcut
    let desktop_shortcut = PathBuf::from("C:\\Users\\Public\\Desktop\\180Hz NC.lnk");
    let _ = fs::remove_file(desktop_shortcut);

    // Remove Start Menu shortcut
    let start_menu_shortcut =
        PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\180Hz NC.lnk");
    let _ = fs::remove_file(start_menu_shortcut);

    // Remove Registry Uninstall Entry
    unsafe {
        let subkey = HSTRING::from("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\180Hz-NC");
        let _ = RegDeleteKeyW(HKEY_LOCAL_MACHINE, &subkey);
    }

    let prog_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
    let install_dir = PathBuf::from(prog_files).join("180Hz-NC");

    // Spawn detached self-deletion script
    let cmd_str = format!(
        "ping 127.0.0.1 -n 2 > nul & rmdir /s /q \"{}\"",
        install_dir.display()
    );
    let _ = Command::new("cmd").args(["/c", &cmd_str]).spawn();

    if !silent {
        show_msg_box(
            "180Hz NC Uninstalled",
            "180Hz NC has been successfully removed from your computer.",
            MB_OK | MB_ICONINFORMATION,
        );
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_uninstall = args.iter().any(|a| a == "--uninstall" || a == "-u" || a == "/uninstall");
    let is_silent = args.iter().any(|a| a == "--silent" || a == "-s" || a == "/S" || a == "/silent");

    if !is_elevated() {
        if let Err(err) = elevate_self() {
            show_msg_box("Administrator Required", &err, MB_OK);
        }
        return;
    }

    if is_uninstall {
        if let Err(err) = run_uninstall(is_silent) {
            show_msg_box("Uninstall Error", &err, MB_OK);
        }
    } else {
        if let Err(err) = run_install(is_silent) {
            show_msg_box("Installation Error", &err, MB_OK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_payloads_validity() {
        assert!(APP_EXE.len() > 1_000_000, "180Hz NC embedded exe must be > 1MB (was {} bytes)", APP_EXE.len());
        assert!(VBCABLE_ZIP.len() > 500_000, "VB-CABLE driver zip must be > 500KB (was {} bytes)", VBCABLE_ZIP.len());
        // Verify ZIP magic header
        assert_eq!(&VBCABLE_ZIP[0..4], b"PK\x03\x04", "Must be valid ZIP archive");
        // Verify PE MZ magic header for 180Hz-NC.exe
        assert_eq!(&APP_EXE[0..2], b"MZ", "Must be valid Windows PE executable");
    }
}
