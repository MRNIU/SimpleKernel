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
    pub dtb_path: PathBuf,
}

/// 准备 QEMU 启动环境（仅需执行一次）
pub fn prepare_qemu_env(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    release: bool,
) -> Result<QemuEnv> {
    firmware::ensure_firmware_ready(sh, project_root, arch)?;
    let boot_dir = build::prepare_boot_directory(project_root, arch, release)?;
    let rootfs_path = build::ensure_rootfs_image(sh, &boot_dir)?;
    let dtb_path = qemu::dump_qemu_dtb(sh, arch, &boot_dir, &rootfs_path)?;
    qemu::generate_boot_script(arch, sh, &boot_dir)?;
    qemu::prepare_boot_part(&boot_dir)?;
    qemu::setup_tftp(&boot_dir);
    Ok(QemuEnv {
        boot_dir,
        rootfs_path,
        dtb_path,
    })
}

/// 表示一个测试二进制——所属包名 + 二进制名。
pub struct TestBinary {
    pub package: String,
    pub bin_name: String,
    /// 用于 `--list` 和 `--name` 的显示名。
    /// 单二进制包：包名本身（如 "device-test"）
    /// 多二进制包：`package/binary`（如 "paging-test/basic"）
    pub display_name: String,
}

/// 运行指定测试
#[expect(
    clippy::too_many_arguments,
    reason = "QEMU 测试需要传递构建和显示相关的多个参数"
)]
pub fn run_test(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    package: &str,
    bin_name: &str,
    display_name: &str,
    env: &QemuEnv,
    release: bool,
    debug_files: bool,
    timeout_secs: u64,
) -> Result<bool> {
    let kernel_elf_path = build::build_binary(
        sh,
        project_root,
        arch,
        Some(package),
        Some(bin_name),
        release,
    )?;
    if debug_files {
        build::generate_debug_files(sh, &kernel_elf_path)?;
    }
    qemu::generate_fit_image(arch, sh, &env.boot_dir, &kernel_elf_path, &env.dtb_path)?;

    println!(
        "[xtask] Running test '{}' (timeout: {}s)...",
        display_name, timeout_secs
    );
    let result = qemu::launch_qemu(
        sh,
        arch,
        project_root,
        &env.boot_dir,
        &kernel_elf_path,
        &env.rootfs_path,
        false,
        Some(timeout_secs),
    );

    match result {
        Ok(()) => {
            println!("[xtask] Test '{}' completed", display_name);
            Ok(true)
        }
        Err(e) => {
            eprintln!("[xtask] Test '{}' failed: {}", display_name, e);
            Ok(false)
        }
    }
}

/// 从 Cargo.toml 内容中解析 [package] name。
///
/// 只匹配 `[[bin]]` 之前出现的第一个 `name = "..."` 行。
fn read_package_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        // 遇到 [[bin]] 前的第一个 name 行即为 package name
        if trimmed == "[[bin]]" {
            break;
        }
        if trimmed.starts_with("name")
            && let Some(start) = trimmed.find('"')
        {
            let rest = &trimmed[start + 1..];
            if let Some(end) = rest.find('"') {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

/// 从 Cargo.toml 中解析所有 `[[bin]]` 的 name 字段。
fn parse_bin_names(content: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_bin = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[bin]]" {
            in_bin = true;
            continue;
        }
        if in_bin
            && trimmed.starts_with("name")
            && let Some(start) = trimmed.find('"')
        {
            let rest = &trimmed[start + 1..];
            if let Some(end) = rest.find('"') {
                names.push(rest[..end].to_string());
                in_bin = false;
            }
        }
        if trimmed.starts_with('[') && trimmed != "[[bin]]" {
            in_bin = false;
        }
    }
    names
}

/// 收集 `tests/` 下所有测试二进制（扫描 `[[bin]]` 条目，跳过库 crate）。
pub fn test_binaries(project_root: &Path) -> Vec<TestBinary> {
    let mut binaries = Vec::new();
    let tests_dir = project_root.join("tests");
    let Ok(entries) = std::fs::read_dir(&tests_dir) else {
        return binaries;
    };
    // 先收集每个包的所有二进制名
    let mut packages: Vec<(String, Vec<String>)> = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        let cargo_toml = dir.join("Cargo.toml");
        if !cargo_toml.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&cargo_toml) else {
            continue;
        };
        let Some(pkg_name) = read_package_name(&content) else {
            continue;
        };
        let bin_names = parse_bin_names(&content);
        if bin_names.is_empty() {
            continue;
        }
        packages.push((pkg_name, bin_names));
    }
    // 生成 TestBinary，单二进制包用包名，多二进制包用 package/binary
    for (pkg_name, bin_names) in packages {
        let single = bin_names.len() == 1 && bin_names[0] == pkg_name;
        for bin_name in bin_names {
            let display_name = if single {
                pkg_name.clone()
            } else {
                format!("{}/{}", pkg_name, bin_name)
            };
            binaries.push(TestBinary {
                package: pkg_name.clone(),
                bin_name,
                display_name,
            });
        }
    }
    binaries
}

/// 列出所有可用测试
pub fn list_tests(project_root: &Path) {
    println!("Available tests:");
    for tb in test_binaries(project_root) {
        println!("  {}", tb.display_name);
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

/// 运行所有测试：构建全部二进制，顺序执行并捕获输出，打印汇总。
///
/// 每个测试在独立 QEMU 实例中运行，输出被捕获而非直接打印到终端。
/// 超时后自动终止 QEMU 进程。顺序复用共享的 `boot/` 目录——每轮
/// 重新生成 FIT 镜像和 TFTP 符号链接，不创建 per-test 子目录。
pub fn run_all_tests(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    env: &QemuEnv,
    release: bool,
    timeout_secs: u64,
    debug_files: bool,
) -> Result<bool> {
    let bins = test_binaries(project_root);

    if bins.is_empty() {
        println!("[xtask] No test binaries found.");
        return Ok(true);
    }

    println!(
        "[xtask] Running {} tests (timeout: {}s each)...",
        bins.len(),
        timeout_secs
    );

    // 阶段 1：编译所有测试二进制（提前暴露编译错误）
    println!("[xtask] Building all test binaries...");
    let mut prepared: Vec<(String, PathBuf)> = Vec::new();
    for tb in &bins {
        let elf = build::build_binary(
            sh,
            project_root,
            arch,
            Some(&tb.package),
            Some(&tb.bin_name),
            release,
        )?;
        if debug_files {
            build::generate_debug_files(sh, &elf)?;
        }
        prepared.push((tb.display_name.clone(), elf));
    }

    // 阶段 2：顺序执行——每轮在共享 boot_dir 重新生成 FIT 和 TFTP
    let mut results: Vec<TestResult> = Vec::new();
    for (name, elf) in &prepared {
        qemu::generate_fit_image(arch, sh, &env.boot_dir, elf, &env.dtb_path)?;

        println!("[xtask] Running test '{name}'...");
        let start = Instant::now();

        let qemu_result = qemu::launch_qemu_captured(
            arch,
            project_root,
            &env.boot_dir,
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
