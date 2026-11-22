use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=launcher.rc");

    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| "target".to_string());
    let res_obj = Path::new(&out_dir).join("launcher_res.o");
    let rc_file = Path::new("launcher.rc");
    if rc_file.exists() {
        let windres_status = Command::new("windres.exe")
            .args(["-i", "launcher.rc", "-o", &res_obj.to_string_lossy()])
            .status();
        if let Ok(st) = windres_status {
            if st.success() && res_obj.exists() {
                println!("cargo:rustc-link-arg={}", res_obj.display());
            }
        }
    }
}
