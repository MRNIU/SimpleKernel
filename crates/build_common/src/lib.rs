//! 内核构建脚本公共逻辑。
//!
//! 将汇编编译和链接器参数设置抽取到此 crate，消除多个 `build.rs` 之间的代码重复。

use std::path::Path;

/// 返回指定架构对应的交叉编译器名称。
fn compiler_for(arch: &str) -> &'static str {
    match arch {
        "riscv64" => "riscv64-linux-gnu-gcc",
        "aarch64" => "aarch64-linux-gnu-gcc",
        _ => unreachable!("不支持的架构: {arch}"),
    }
}

/// 返回指定架构需要的额外编译标志。
fn extra_flags_for(arch: &str) -> &'static [&'static str] {
    match arch {
        "riscv64" => &["-march=rv64gc", "-mabi=lp64d"],
        _ => &[],
    }
}

/// 编译架构相关的汇编文件并设置链接器参数。
///
/// `arch_dir` 为 `src/arch/{riscv64|aarch64}/` 的绝对或相对路径。
/// 函数会自动从 `CARGO_CFG_TARGET_ARCH` 环境变量获取目标架构。
pub fn setup_kernel_build(arch_dir: &Path) {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH 未设置");

    // ── 编译汇编文件 ──────────────────────────────────────────────────
    let mut build = cc::Build::new();
    build.compiler(compiler_for(&arch));

    for flag in extra_flags_for(&arch) {
        build.flag(flag);
    }

    build.include(arch_dir);

    // 自动发现 .S 文件，新增汇编源文件无需手动注册。
    let mut has_asm = false;
    for entry in std::fs::read_dir(arch_dir)
        .unwrap_or_else(|e| panic!("无法读取目录 {}: {e}", arch_dir.display()))
    {
        let path = entry
            .unwrap_or_else(|e| panic!("无法读取目录条目 {}: {e}", arch_dir.display()))
            .path();
        if path.extension().is_some_and(|ext| ext == "S") {
            build.file(&path);
            has_asm = true;
        }
    }

    if has_asm {
        build.compile("asm");
    }

    // ── 链接器参数 ───────────────────────────────────────────────────
    println!("cargo:rustc-link-arg=-z");
    println!("cargo:rustc-link-arg=norelro");
    println!(
        "cargo:rustc-link-arg=-T{}",
        arch_dir.join("link.ld").display()
    );

    // 监视整个架构目录，头文件/链接脚本/新增 .S 的变更都会触发重建。
    println!("cargo:rerun-if-changed={}", arch_dir.display());
}
