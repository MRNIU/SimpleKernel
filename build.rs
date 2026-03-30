use std::path::PathBuf;

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH 未设置");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if !matches!(arch.as_str(), "riscv64" | "aarch64") || os != "none" {
        return;
    }

    let arch_dir = PathBuf::from("src/arch").join(&arch);
    build_common::setup_kernel_build(&arch_dir);
}
