use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=resources/app.ico");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let out_dir = env::var("OUT_DIR").unwrap();
        let rc_path = Path::new("resources/app.rc");
        let res_path = Path::new(&out_dir).join("app.o");

        let mut windres_candidates = Vec::new();
        if let Ok(w) = env::var("WINDRES") {
            windres_candidates.push(w);
        }
        if let Ok(w64) = env::var("W64DEVKIT_HOME") {
            windres_candidates.push(format!("{}\\bin\\windres.exe", w64));
        }
        if let Ok(mingw) = env::var("MINGW_HOME") {
            windres_candidates.push(format!("{}\\bin\\windres.exe", mingw));
        }
        let user_profile = env::var("USERPROFILE").unwrap_or_default();
        if !user_profile.is_empty() {
            windres_candidates.push(format!("{}\\w64devkit\\bin\\windres.exe", user_profile));
        }
        windres_candidates.push("windres.exe".to_string());
        windres_candidates.push("windres".to_string());

        let windres_bin = windres_candidates
            .into_iter()
            .find(|p| !p.is_empty() && (Path::new(p).exists() || Command::new(p).arg("--version").output().is_ok()))
            .unwrap_or_else(|| "windres".to_string());

        let status = Command::new(&windres_bin)
            .arg(rc_path)
            .arg("-I")
            .arg("resources")
            .arg("-O")
            .arg("coff")
            .arg("-o")
            .arg(&res_path)
            .status();

        if let Ok(s) = status {
            if s.success() {
                println!("cargo:rustc-link-arg={}", res_path.display());
            } else {
                eprintln!("windres exit status error: {:?}", s.code());
            }
        } else if let Err(e) = status {
            eprintln!("windres execution error (tried {}): {}", windres_bin, e);
        }
    }
}
