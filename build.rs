use std::path::PathBuf;

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH 未设置");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if !matches!(arch.as_str(), "riscv64" | "aarch64") || os != "none" {
        return;
    }

    let arch_dir = PathBuf::from("src/arch").join(&arch);
    compile_assembly(&arch, &arch_dir);
}

/// 编译架构相关的汇编文件。
///
/// 链接器参数（`-T link.ld`、`-z norelro`）已移至 `.cargo/config.toml`，
/// 对 workspace 内所有裸机二进制统一生效，无需每个 crate 重复设置。
fn compile_assembly(arch: &str, arch_dir: &std::path::Path) {
    let compiler = match arch {
        "riscv64" => "riscv64-linux-gnu-gcc",
        "aarch64" => "aarch64-linux-gnu-gcc",
        _ => unreachable!("不支持的架构: {arch}"),
    };

    let mut build = cc::Build::new();
    build.compiler(compiler);

    if arch == "riscv64" {
        build.flag("-march=rv64gc").flag("-mabi=lp64d");
    }

    build.include(arch_dir);

    let mut has_asm = false;
    for entry in std::fs::read_dir(arch_dir)
        .unwrap_or_else(|e| panic!("无法读取目录 {}: {e}", arch_dir.display()))
    {
        let path = entry
            .unwrap_or_else(|e| panic!("无法读取目录条目 {}: {e}", arch_dir.display()))
            .path();
        if path.extension().is_some_and(|ext| ext == "S") {
            println!("cargo:rerun-if-changed={}", path.display());
            build.file(&path);
            has_asm = true;
        }
    }

    if has_asm {
        build.compile("asm");
    }

    println!("cargo:rerun-if-changed={}", arch_dir.display());
}
