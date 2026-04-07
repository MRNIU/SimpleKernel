//! `cargo xtask test` — 在 QEMU 中运行系统测试

use std::path::{Path, PathBuf};
use std::time::Instant;
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

/// 运行指定测试
pub fn run_test(
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

/// 收集 `tests/` 下所有测试二进制的包名（跳过库 crate）
pub fn test_packages(project_root: &Path) -> Vec<String> {
    let mut packages = Vec::new();
    let tests_dir = project_root.join("tests");
    if tests_dir.exists()
        && let Ok(entries) = std::fs::read_dir(&tests_dir)
    {
        for entry in entries.flatten() {
            let dir = entry.path();
            let cargo_toml = dir.join("Cargo.toml");
            if cargo_toml.exists()
                && dir.join("src/main.rs").exists()
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
    for name in test_packages(project_root) {
        println!("  {name}");
    }
}

/// 单个测试的执行结果
struct TestResult {
    name: String,
    passed: bool,
    timed_out: bool,
    duration: std::time::Duration,
    output: String,
}

/// 为指定测试包创建独立的 boot 子目录，避免多个测试共享 FIT 镜像文件。
fn per_test_boot_dir(base_boot_dir: &Path, test_name: &str) -> Result<PathBuf> {
    let dir = base_boot_dir.join(test_name);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 构建单个测试二进制，生成对应的 FIT 镜像到独立 boot 子目录。
///
/// 返回 (kernel_elf_path, per_test_boot_dir)。
fn build_test_with_fit(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    package: &str,
    env: &QemuEnv,
    release: bool,
) -> Result<(PathBuf, PathBuf)> {
    let kernel_elf_path = build::build_binary(sh, project_root, arch, Some(package), release)?;
    build::generate_debug_files(sh, &kernel_elf_path)?;

    let test_boot_dir = per_test_boot_dir(&env.boot_dir, package)?;

    // 生成此测试专用的 FIT 镜像
    qemu::generate_fit_image(arch, sh, &test_boot_dir, &kernel_elf_path, &env.dtb_path)?;
    // 生成此测试专用的 boot script
    qemu::generate_boot_script(arch, sh, &test_boot_dir)?;
    // 为此测试设置 TFTP（顺序执行时安全）
    qemu::setup_tftp(&test_boot_dir);

    Ok((kernel_elf_path, test_boot_dir))
}

/// 运行所有测试：构建全部二进制，顺序执行并捕获输出，打印汇总。
///
/// 每个测试在独立 QEMU 实例中运行，输出被捕获而非直接打印到终端。
/// 超时后自动终止 QEMU 进程。
// TODO: 未来支持 --jobs 并行执行（需解决 TFTP 目录共享冲突）
pub fn run_all_tests(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    env: &QemuEnv,
    release: bool,
    timeout_secs: u64,
) -> Result<bool> {
    let packages: Vec<String> = test_packages(project_root);

    if packages.is_empty() {
        println!("[xtask] No test packages found.");
        return Ok(true);
    }

    println!(
        "[xtask] Running {} tests (timeout: {}s each)...",
        packages.len(),
        timeout_secs
    );

    // 阶段 1：构建所有测试二进制并生成 FIT 镜像
    println!("[xtask] Building all test binaries...");
    let mut prepared: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    for name in &packages {
        let (elf, boot_dir) = build_test_with_fit(sh, project_root, arch, name, env, release)?;
        prepared.push((name.clone(), elf, boot_dir));
    }

    // 阶段 2：顺序执行每个测试
    let mut results: Vec<TestResult> = Vec::new();
    for (name, elf, test_boot_dir) in &prepared {
        println!("[xtask] Running test '{name}'...");
        let start = Instant::now();

        let qemu_result = qemu::launch_qemu_captured(
            arch,
            project_root,
            test_boot_dir,
            elf,
            &env.rootfs_path,
            timeout_secs,
        )?;

        let duration = start.elapsed();
        let status = if qemu_result.timed_out {
            "TIMEOUT"
        } else if qemu_result.success {
            "PASS"
        } else {
            "FAIL"
        };
        println!(
            "[xtask]   {name}: {status} ({:.1}s)",
            duration.as_secs_f64()
        );

        results.push(TestResult {
            name: name.clone(),
            passed: qemu_result.success,
            timed_out: qemu_result.timed_out,
            duration,
            output: qemu_result.output,
        });
    }

    // 阶段 3：打印汇总
    println!();
    println!("========== Test Summary ==========");
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut timed_out = 0u32;
    for r in &results {
        let status = if r.timed_out {
            timed_out += 1;
            "TIMEOUT"
        } else if r.passed {
            passed += 1;
            "PASS"
        } else {
            failed += 1;
            "FAIL"
        };
        println!(
            "  {}: {} ({:.1}s)",
            r.name,
            status,
            r.duration.as_secs_f64()
        );
    }
    println!(
        "  Total: {} passed, {} failed, {} timed out",
        passed, failed, timed_out
    );
    println!("==================================");

    // 打印失败测试的输出
    for r in &results {
        if !r.passed {
            println!();
            println!("---------- {} output ----------", r.name);
            println!("{}", r.output);
            println!("---------- end {} ----------", r.name);
        }
    }

    Ok(failed == 0 && timed_out == 0)
}
