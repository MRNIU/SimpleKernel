//! xtask — 内核构建工具
//!
//! 替代 CMake 的宿主机构建脚本，通过 `cargo xtask <subcommand>` 调用。
//!
//! 子命令：
//! - `build`    — 编译内核并生成调试文件
//! - `run`      — 编译并在 QEMU 中运行
//! - `debug`    — 编译并在 QEMU 中以调试模式运行（暂停 CPU，等待 GDB 连接）
//! - `firmware` — 编译第三方固件（OpenSBI / U-Boot / OP-TEE / ATF）

mod arch;
mod build;
mod firmware;
mod qemu;
mod test;

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::process;

pub use arch::Arch;

type DynError = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, DynError>;

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args, Clone)]
struct ArchArgs {
    #[arg(long, value_enum, default_value = "riscv64")]
    arch: Arch,
    #[arg(long)]
    release: bool,
}

#[derive(Args, Clone)]
struct TestArgs {
    #[arg(long, value_enum, default_value = "riscv64")]
    arch: Arch,
    #[arg(long)]
    release: bool,
    /// 运行指定的独立测试
    #[arg(long)]
    name: Option<String>,
    /// 运行全部测试（统一 + 所有独立）
    #[arg(long)]
    all: bool,
    /// 列出可用测试
    #[arg(long)]
    list: bool,
}

#[derive(Subcommand)]
enum Commands {
    Build(ArchArgs),
    /// 检查编译（等价于 `cargo check --target <target> -Z build-std=...`）
    Check(ArchArgs),
    Run(ArchArgs),
    /// 启动 QEMU 并暂停 CPU，等待 GDB 在 localhost:1234 连接
    Debug(ArchArgs),
    Firmware(ArchArgs),
    /// 在 QEMU 中运行系统测试
    Test(TestArgs),
}

fn main() {
    if let Err(error) = run() {
        eprintln!("[xtask] error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let project_root = project_root();
    let sh = xshell::Shell::new()?;
    sh.change_dir(&project_root);

    match cli.command {
        Commands::Build(args) => {
            let kernel_elf_path =
                build::build_binary(&sh, &project_root, args.arch, None, args.release)?;
            build::generate_debug_files(&sh, &kernel_elf_path)?;
        }
        Commands::Check(args) => {
            build::check_target(&sh, args.arch)?;
        }
        Commands::Firmware(args) => {
            firmware::build_firmware(&sh, &project_root, args.arch)?;
        }
        Commands::Run(args) => {
            let arch = args.arch;
            // 提前检查固件，避免内核编译完成后才发现固件缺失。
            firmware::ensure_firmware_exists(&project_root, arch)?;
            let kernel_elf_path =
                build::build_binary(&sh, &project_root, arch, None, args.release)?;
            build::generate_debug_files(&sh, &kernel_elf_path)?;
            let boot_dir = build::prepare_boot_directory(&project_root, arch, args.release)?;
            let rootfs_path = build::ensure_rootfs_image(&sh, &boot_dir)?;
            let dtb_path = qemu::dump_qemu_dtb(&sh, arch, &boot_dir, &rootfs_path)?;
            qemu::generate_fit_image(arch, &sh, &boot_dir, &kernel_elf_path, &dtb_path)?;
            qemu::generate_boot_script(arch, &sh, &boot_dir)?;
            qemu::setup_tftp(&boot_dir);
            qemu::launch_qemu(
                &sh,
                arch,
                &project_root,
                &boot_dir,
                &kernel_elf_path,
                &rootfs_path,
                false,
            )?;
        }
        Commands::Debug(args) => {
            let arch = args.arch;
            firmware::ensure_firmware_exists(&project_root, arch)?;
            let kernel_elf_path =
                build::build_binary(&sh, &project_root, arch, None, args.release)?;
            build::generate_debug_files(&sh, &kernel_elf_path)?;
            let boot_dir = build::prepare_boot_directory(&project_root, arch, args.release)?;
            let rootfs_path = build::ensure_rootfs_image(&sh, &boot_dir)?;
            let dtb_path = qemu::dump_qemu_dtb(&sh, arch, &boot_dir, &rootfs_path)?;
            qemu::generate_fit_image(arch, &sh, &boot_dir, &kernel_elf_path, &dtb_path)?;
            qemu::generate_boot_script(arch, &sh, &boot_dir)?;
            qemu::setup_tftp(&boot_dir);
            qemu::launch_qemu(
                &sh,
                arch,
                &project_root,
                &boot_dir,
                &kernel_elf_path,
                &rootfs_path,
                true,
            )?;
        }
        Commands::Test(args) => {
            if args.list {
                test::list_tests(&project_root);
                return Ok(());
            }
            // 准备 QEMU 环境（固件、boot 目录、rootfs、DTB、boot script），仅执行一次
            let qemu_env = test::prepare_qemu_env(&sh, &project_root, args.arch, args.release)?;
            let mut all_passed = true;
            if let Some(name) = &args.name {
                let passed = test::run_standalone_test(
                    &sh,
                    &project_root,
                    args.arch,
                    name,
                    &qemu_env,
                    args.release,
                )?;
                all_passed &= passed;
            } else {
                let passed =
                    test::run_system_test(&sh, &project_root, args.arch, &qemu_env, args.release)?;
                all_passed &= passed;
            }
            if args.all {
                for name in test::standalone_test_packages(&project_root) {
                    let passed = test::run_standalone_test(
                        &sh,
                        &project_root,
                        args.arch,
                        &name,
                        &qemu_env,
                        args.release,
                    )?;
                    all_passed &= passed;
                }
            }
            if !all_passed {
                eprintln!("[xtask] Some tests failed");
                process::exit(1);
            }
            println!("[xtask] All tests passed");
        }
    }

    Ok(())
}

fn project_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a direct workspace member")
        .to_path_buf()
}
