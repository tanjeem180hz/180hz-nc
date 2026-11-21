use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=installer.rc");
    println!("cargo:rerun-if-changed=resources/installer.manifest");
    println!("cargo:rerun-if-changed=resources/icon.ico");
    println!("cargo:rerun-if-changed=resources/app.exe");
    println!("cargo:rerun-if-changed=resources/WebView2Loader.dll");
    println!("cargo:rerun-if-changed=resources/vbcable_pack.zip");
    println!("cargo:rerun-if-changed=resources/180hz_cert.cer");

    let resources_dir = Path::new("resources");
    let _ = fs::create_dir_all(resources_dir);

    let app_target = resources_dir.join("app.exe");
    let release_candidates = [
        Path::new("../../target/release/quietmic-app.exe"),
        Path::new("../../target/release/180hz nc.exe"),
    ];
    let mut copied = false;
    for cand in &release_candidates {
        if cand.exists() {
            let should_copy = if app_target.exists() {
                let cand_len = fs::metadata(cand).map(|m| m.len()).unwrap_or(0);
                let target_len = fs::metadata(&app_target).map(|m| m.len()).unwrap_or(0);
                cand_len != target_len || cand_len == 0
            } else {
                true
            };
            if should_copy {
                let _ = fs::copy(cand, &app_target);
            }
            copied = true;
            break;
        }
    }
    if !copied && !app_target.exists() {
        // Build quietmic-app if missing
        println!("cargo:warning=Building quietmic-app for embedding into installer...");
        let status = Command::new("cargo")
            .args(["build", "--release", "-p", "quietmic-app"])
            .status();
        if let Ok(st) = status {
            if st.success() {
                for cand in &release_candidates {
                    if cand.exists() {
                        let _ = fs::copy(cand, &app_target);
                        break;
                    }
                }
            }
        }
    }

    // Ensure app.exe exists for compilation
    if !app_target.exists() {
        let _ = fs::write(&app_target, b"180HZ_NC_STUB_EXE");
    }

    let webview2_target = resources_dir.join("WebView2Loader.dll");
    let webview2_candidates = [
        Path::new("../../target/release/WebView2Loader.dll"),
        Path::new("../../target/debug/WebView2Loader.dll"),
    ];
    let mut wv2_copied = false;
    for cand in &webview2_candidates {
        if cand.exists() {
            let should_copy = if webview2_target.exists() {
                let cand_len = fs::metadata(cand).map(|m| m.len()).unwrap_or(0);
                let target_len = fs::metadata(&webview2_target).map(|m| m.len()).unwrap_or(0);
                cand_len != target_len || cand_len == 0
            } else {
                true
            };
            if should_copy {
                let _ = fs::copy(cand, &webview2_target);
            }
            wv2_copied = true;
            break;
        }
    }
    if !wv2_copied && !webview2_target.exists() {
        let _ = fs::write(&webview2_target, b"WEBVIEW2_LOADER_STUB");
    }

    let vbcable_target = resources_dir.join("vbcable_pack.zip");
    if !vbcable_target.exists() {
        let root_pack = Path::new("../../vbcable_pack.zip");
        if root_pack.exists() {
            let _ = fs::copy(root_pack, &vbcable_target);
        } else {
            println!("cargo:warning=Downloading VB-CABLE driver pack...");
            let _ = Command::new("curl.exe")
                .args([
                    "-L",
                    "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip",
                    "-o",
                    &vbcable_target.to_string_lossy(),
                    "--user-agent",
                    "Mozilla/5.0",
                ])
                .status();
        }
    }

    // Ensure vbcable_pack.zip exists for compilation
    if !vbcable_target.exists() {
        let _ = fs::write(&vbcable_target, b"VBCABLE_STUB_ZIP");
    }

    // Ensure cert exists
    let cert_target = resources_dir.join("180hz_cert.cer");
    if !cert_target.exists() {
        let root_cert = Path::new("../../target/release/180hz_cert.cer");
        if root_cert.exists() {
            let _ = fs::copy(root_cert, &cert_target);
        } else {
            let _ = fs::write(&cert_target, b"180HZ_CERT_STUB");
        }
    }

    // Compile PE version info & icon into resource object via windres
    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| "target".to_string());
    let res_obj = Path::new(&out_dir).join("installer_res.o");
    let rc_file = Path::new("installer.rc");
    if rc_file.exists() {
        let windres_status = Command::new("windres.exe")
            .args(["-i", "installer.rc", "-o", &res_obj.to_string_lossy()])
            .status();
        if let Ok(st) = windres_status {
            if st.success() && res_obj.exists() {
                println!("cargo:rustc-link-arg={}", res_obj.display());
            }
        }
    }
}
