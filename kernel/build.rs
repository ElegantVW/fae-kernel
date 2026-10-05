fn main() {
    println!("cargo:rerun-if-changed=../fw/font8x16.bin");
    println!("cargo:rerun-if-changed=../scripts/mkfont.py");
    let font = std::path::Path::new("../fw/font8x16.bin");
    if !font.exists() {
        let status = std::process::Command::new("python3")
            .arg("../scripts/mkfont.py")
            .status();
        let _ = status;
    }
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if os == "uefi" {
        return;
    }
    let fw = std::env::var("CARGO_FEATURE_FW_LINK").is_ok();
    let script = if fw {
        "linker-fw.ld".into()
    } else {
        format!("linker-{arch}.ld")
    };
    println!("cargo:rustc-link-arg=-T{script}");
    println!("cargo:rerun-if-changed={script}");
}
