use std::path::PathBuf;

fn compiler_for(arch: &str) -> &'static str {
    match arch {
        "riscv64" => "riscv64-linux-gnu-gcc",
        "aarch64" => "aarch64-linux-gnu-gcc",
        _ => unreachable!("unsupported architecture: {arch}"),
    }
}

fn extra_flags_for(arch: &str) -> &'static [&'static str] {
    match arch {
        "riscv64" => &["-march=rv64gc", "-mabi=lp64d"],
        _ => &[],
    }
}

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if !matches!(arch.as_str(), "riscv64" | "aarch64") || os != "none" {
        return;
    }

    let arch_dir = PathBuf::from("src/arch").join(&arch);

    // ── 编译汇编文件 ──────────────────────────────────────────────────
    let mut build = cc::Build::new();
    build.compiler(compiler_for(&arch));

    for flag in extra_flags_for(&arch) {
        build.flag(flag);
    }

    build.include(&arch_dir);

    // 自动发现 .S 文件，新增汇编源文件无需手动注册。
    let mut has_asm = false;
    for entry in std::fs::read_dir(&arch_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", arch_dir.display()))
    {
        let path = entry
            .unwrap_or_else(|e| panic!("cannot read entry in {}: {e}", arch_dir.display()))
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
