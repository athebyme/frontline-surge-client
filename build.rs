use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let target = env::var("TARGET").expect("Cargo target");
    if !target.contains("windows") || env::var_os("CARGO_FEATURE_CLIENT").is_none() {
        return;
    }
    const SOURCE: &str = "assets/windows.rc";
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed={SOURCE}");
    println!("cargo:rerun-if-changed=assets/frontline.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");
    let version = env!("CARGO_PKG_VERSION");
    let numeric = version.replace('.', ",") + ",0";
    let resource = fs::read_to_string(SOURCE).expect("Windows version resource");
    for field in [
        format!("FILEVERSION {numeric}"),
        format!("PRODUCTVERSION {numeric}"),
        format!(r#""FileVersion", "{version}""#),
        format!(r#""ProductVersion", "{version}""#),
    ] {
        assert!(
            resource.contains(&field),
            "Update {SOURCE} for version {version}"
        );
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo build directory"));
    let (object, result) = if target.ends_with("gnu") {
        let object = out.join("client-resource.o");
        let tool = env::var("WINDRES").unwrap_or_else(|_| {
            if cfg!(windows) {
                "windres".into()
            } else {
                "x86_64-w64-mingw32-windres".into()
            }
        });
        let result = Command::new(tool)
            .args(["-I.", "-i", SOURCE, "-O", "coff", "-o"])
            .arg(&object)
            .status();
        (object, result)
    } else {
        let object = out.join("client-resource.res");
        let result = Command::new("rc.exe")
            .args(["/nologo", "/fo"])
            .arg(&object)
            .arg(SOURCE)
            .status();
        (object, result)
    };
    match result {
        Ok(status) if status.success() => println!("cargo:rustc-link-arg-bin=frontline-surge={}", object.display()),
        _ => println!("cargo:warning=Windows resource compiler unavailable; continuing without file metadata."),
    }
}
