fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if os == "uefi" {
        return;
    }
    let bin = std::env::var("CARGO_BIN_NAME").unwrap_or_default();
    let script = if bin == "fae-kernel-fw" {
        "linker-fw.ld".into()
    } else {
        format!("linker-{arch}.ld")
    };
    println!("cargo:rustc-link-arg=-T{script}");
    println!("cargo:rerun-if-changed={script}");
}
