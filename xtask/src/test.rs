//! `cargo xtask test` — 在 QEMU 中运行系统测试

use std::path::{Path, PathBuf};
use xshell::Shell;

use crate::arch::Arch;
use crate::{Result, build, firmware, qemu};

/// QEMU 启动所需的共享环境（与具体测试二进制无关）
pub struct QemuEnv {
    pub boot_dir: PathBuf,
    pub rootfs_path: PathBuf,
    #[allow(dead_code)] // dtb_path 在 prepare 阶段被 generate_fit_image 间接使用
    pub dtb_path: PathBuf,
}

/// 准备 QEMU 启动环境（仅需执行一次）
pub fn prepare_qemu_env(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    release: bool,
) -> Result<QemuEnv> {
    firmware::ensure_firmware_exists(project_root, arch)?;
    let boot_dir = build::prepare_boot_directory(project_root, arch, release)?;
    let rootfs_path = build::ensure_rootfs_image(sh, &boot_dir)?;
    let dtb_path = qemu::dump_qemu_dtb(sh, arch, &boot_dir, &rootfs_path)?;
    qemu::generate_boot_script(arch, sh, &boot_dir)?;
    qemu::setup_tftp(&boot_dir);
    Ok(QemuEnv {
        boot_dir,
        rootfs_path,
        dtb_path,
    })
}

/// 运行统一系统测试
pub fn run_system_test(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    env: &QemuEnv,
    release: bool,
) -> Result<bool> {
    run_test_binary(sh, project_root, arch, "system-test", env, release)
}

/// 运行指定的独立测试
pub fn run_standalone_test(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    name: &str,
    env: &QemuEnv,
    release: bool,
) -> Result<bool> {
    run_test_binary(sh, project_root, arch, name, env, release)
}

fn run_test_binary(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    package: &str,
    env: &QemuEnv,
    release: bool,
) -> Result<bool> {
    let kernel_elf_path = build::build_binary(sh, project_root, arch, Some(package), release)?;
    build::generate_debug_files(sh, &kernel_elf_path)?;
    qemu::generate_fit_image(arch, sh, &env.boot_dir, &kernel_elf_path, &env.dtb_path)?;

    println!("[xtask] Running test '{}'...", package);
    let result = qemu::launch_qemu(
        sh,
        arch,
        project_root,
        &env.boot_dir,
        &kernel_elf_path,
        &env.rootfs_path,
        false,
    );

    match result {
        Ok(()) => {
            println!("[xtask] Test '{}' completed", package);
            Ok(true)
        }
        Err(e) => {
            eprintln!("[xtask] Test '{}' failed: {}", package, e);
            Ok(false)
        }
    }
}

/// 从 Cargo.toml 中读取 package name
fn read_package_name(cargo_toml: &Path) -> Option<String> {
    let content = std::fs::read_to_string(cargo_toml).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name") {
            // 解析 name = "xxx" 或 name = 'xxx'
            if let Some(start) = trimmed.find('"') {
                let rest = &trimmed[start + 1..];
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_string());
                }
            }
        }
    }
    None
}

/// 收集所有独立测试的包名
pub fn standalone_test_packages(project_root: &Path) -> Vec<String> {
    let mut packages = Vec::new();
    let standalone_dir = project_root.join("tests/standalone");
    if standalone_dir.exists()
        && let Ok(entries) = std::fs::read_dir(&standalone_dir)
    {
        for entry in entries.flatten() {
            let cargo_toml = entry.path().join("Cargo.toml");
            if cargo_toml.exists()
                && let Some(name) = read_package_name(&cargo_toml)
            {
                packages.push(name);
            }
        }
    }
    packages
}

/// 列出所有可用测试
pub fn list_tests(project_root: &Path) {
    println!("Available tests:");
    println!("  system-test      — Unified system test kernel (all groups)");
    for name in standalone_test_packages(project_root) {
        println!("  {name}      — Standalone test");
    }
}
