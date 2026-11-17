use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/app.exe");
    println!("cargo:rerun-if-changed=resources/vbcable_pack.zip");

    let resources_dir = Path::new("resources");
    let _ = fs::create_dir_all(resources_dir);

    let app_target = resources_dir.join("app.exe");
    if !app_target.exists() {
        let release_exe = Path::new("../../target/release/quietmic-app.exe");
        if release_exe.exists() {
            let _ = fs::copy(release_exe, &app_target);
        } else {
            // Build quietmic-app if missing
            println!("cargo:warning=Building quietmic-app for embedding into installer...");
            let status = Command::new("cargo")
                .args(["build", "--release", "-p", "quietmic-app"])
                .status();
            if let Ok(st) = status {
                if st.success() && release_exe.exists() {
                    let _ = fs::copy(release_exe, &app_target);
                }
            }
        }
    }

    // Ensure app.exe exists for compilation
    if !app_target.exists() {
        let _ = fs::write(&app_target, b"180HZ_NC_STUB_EXE");
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
}
