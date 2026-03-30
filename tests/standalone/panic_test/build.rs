use std::path::PathBuf;

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH 未设置");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if !matches!(arch.as_str(), "riscv64" | "aarch64") || os != "none" {
        return;
    }

    // CARGO_MANIFEST_DIR 指向 tests/standalone/panic_test/，向上三级到达项目根目录
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 未设置"));
    let arch_dir = manifest_dir
        .join("../../../src/arch")
        .join(&arch)
        .canonicalize()
        .unwrap_or_else(|e| {
            panic!(
                "无法规范化架构目录路径 {}: {e}",
                manifest_dir.join("../../../src/arch").join(&arch).display()
            )
        });

    build_common::setup_kernel_build(&arch_dir);
}
