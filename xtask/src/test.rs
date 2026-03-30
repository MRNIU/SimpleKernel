//! `cargo xtask test` — 在 QEMU 中运行系统测试

use std::path::Path;
use xshell::Shell;

use crate::arch::Arch;
use crate::{Result, build, firmware, qemu};

/// 运行统一系统测试
pub fn run_system_test(sh: &Shell, project_root: &Path, arch: Arch, release: bool) -> Result<bool> {
    run_test_binary(sh, project_root, arch, "system-test", release)
}

/// 运行指定的独立测试
pub fn run_standalone_test(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    name: &str,
    release: bool,
) -> Result<bool> {
    run_test_binary(sh, project_root, arch, name, release)
}

fn run_test_binary(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    package: &str,
    release: bool,
) -> Result<bool> {
    firmware::ensure_firmware_exists(project_root, arch)?;
    let kernel_elf_path = build::build_test_kernel(sh, project_root, arch, package, release)?;
    build::generate_debug_files(sh, &kernel_elf_path)?;
    let boot_dir = build::prepare_boot_directory(project_root, arch, release)?;
    let rootfs_path = build::ensure_rootfs_image(sh, &boot_dir)?;
    let dtb_path = qemu::dump_qemu_dtb(sh, arch, &boot_dir, &rootfs_path)?;
    qemu::generate_fit_image(arch, sh, &boot_dir, &kernel_elf_path, &dtb_path)?;
    qemu::generate_boot_script(arch, sh, &boot_dir)?;
    qemu::setup_tftp(&boot_dir);

    println!("[xtask] Running test '{}'...", package);
    let result = qemu::launch_qemu(
        sh,
        arch,
        project_root,
        &boot_dir,
        &kernel_elf_path,
        &rootfs_path,
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
    if standalone_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&standalone_dir) {
            for entry in entries.flatten() {
                let cargo_toml = entry.path().join("Cargo.toml");
                if cargo_toml.exists() {
                    if let Some(name) = read_package_name(&cargo_toml) {
                        packages.push(name);
                    }
                }
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
