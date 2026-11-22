#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const WEBVIEW2_LOADER: &[u8] = include_bytes!("../../../release/WebView2Loader.dll");
const APP_EXE: &[u8] = include_bytes!("../../../release/180hz-nc.exe");

fn main() {
    let local_dir = match env::var("LOCALAPPDATA") {
        Ok(val) => PathBuf::from(val).join("180hz nc"),
        Err(_) => match env::var("USERPROFILE") {
            Ok(val) => PathBuf::from(val).join("AppData\\Local\\180hz nc"),
            Err(_) => PathBuf::from("."),
        },
    };

    let _ = fs::create_dir_all(&local_dir);

    // 1. Ensure WebView2Loader.dll is in LocalAppData
    let target_dll = local_dir.join("WebView2Loader.dll");
    let needs_dll_write = match fs::metadata(&target_dll) {
        Ok(m) => m.len() != WEBVIEW2_LOADER.len() as u64,
        Err(_) => true,
    };
    if needs_dll_write {
        let _ = fs::write(&target_dll, WEBVIEW2_LOADER);
    }

    // 2. Ensure quietmic-app.exe is in LocalAppData
    let target_app = local_dir.join("quietmic-app.exe");
    let needs_app_write = match fs::metadata(&target_app) {
        Ok(m) => m.len() != APP_EXE.len() as u64,
        Err(_) => true,
    };
    if needs_app_write {
        let _ = fs::write(&target_app, APP_EXE);
    }

    // 3. If running from another directory, also try writing WebView2Loader.dll there
    if let Ok(cur_exe) = env::current_exe() {
        if let Some(parent) = cur_exe.parent() {
            if parent != local_dir {
                let local_dll = parent.join("WebView2Loader.dll");
                if !local_dll.exists() {
                    let _ = fs::write(&local_dll, WEBVIEW2_LOADER);
                }
            }
        }
    }

    // 4. Spawn the actual application
    let args: Vec<String> = env::args().skip(1).collect();
    let is_cli = args.iter().any(|a| {
        a == "--help" || a == "-h" || a == "--version" || a == "-V" || a == "status" || a == "--cli"
    });

    let mut cmd = Command::new(&target_app);
    cmd.args(&args);
    cmd.current_dir(&local_dir);

    if is_cli {
        match cmd.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(0)),
            Err(_) => std::process::exit(1),
        }
    } else {
        let _ = cmd.spawn();
    }
}
