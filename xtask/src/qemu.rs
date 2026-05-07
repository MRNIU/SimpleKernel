use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use xshell::{Cmd, Shell, cmd};

use crate::Result;
use crate::arch::Arch;

// boot.its.template 在编译时嵌入，避免运行时文件路径依赖。
const ITS_TEMPLATE: &str = include_str!("boot.its.template");

// 将架构相关参数填入模板，生成完整的 ITS 文件内容。
fn generate_its_content(arch: Arch, kernel_path: &str, dtb_path: &str) -> String {
    let (fit_arch, load_addr) = match arch {
        Arch::Riscv64 => ("riscv", "0x80200000"),
        Arch::Aarch64 => ("arm64", "0x40100000"),
    };
    ITS_TEMPLATE
        .replace("{KERNEL_PATH}", kernel_path)
        .replace("{DTB_PATH}", dtb_path)
        .replace("{FIT_ARCH}", fit_arch)
        .replace("{LOAD_ADDR}", load_addr)
}

// 所有 QEMU 调用共享的基础参数：无图形、1 GiB 内存、2 核、
// virtio 网络/GPU/块设备、machine 和 cpu 按架构选择。
fn base_qemu_args(arch: Arch, rootfs_drive: &str) -> Vec<String> {
    let (machine, cpu) = match arch {
        Arch::Riscv64 => ("virt", "max"),
        Arch::Aarch64 => ("virt,secure=on,gic_version=3", "cortex-a72"),
    };
    [
        "-nographic",
        "-monitor",
        "telnet::2333,server,nowait",
        "-m",
        "1024M",
        "-smp",
        "2",
        "-global",
        "virtio-mmio.force-legacy=false",
        "-netdev",
        "user,id=net0,tftp=/srv/tftp,bootfile=boot.scr.uimg",
        "-device",
        "virtio-net-device,netdev=net0",
        "-device",
        "virtio-gpu-device",
        "-drive",
        rootfs_drive,
        "-device",
        "virtio-blk-device,drive=hd0",
        "-machine",
        machine,
        "-cpu",
        cpu,
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn base_qemu_cmd<'a>(sh: &'a Shell, arch: Arch, rootfs_drive: &str) -> Cmd<'a> {
    sh.cmd(arch.qemu_binary())
        .args(base_qemu_args(arch, rootfs_drive))
}

fn base_qemu_timeout_cmd<'a>(
    sh: &'a Shell,
    arch: Arch,
    rootfs_drive: &str,
    timeout_secs: u64,
) -> Cmd<'a> {
    let timeout = format!("{timeout_secs}s");
    sh.cmd("timeout")
        .arg(timeout)
        .arg(arch.qemu_binary())
        .args(base_qemu_args(arch, rootfs_drive))
}

/// 启动 QEMU 并将硬件设备树导出到 `boot_dir/qemu.dtb`。
pub fn dump_qemu_dtb(
    sh: &Shell,
    arch: Arch,
    boot_dir: &Path,
    rootfs_path: &Path,
) -> Result<PathBuf> {
    let dtb_path = boot_dir.join("qemu.dtb");
    println!("[xtask] Generating QEMU DTB at {}...", dtb_path.display());

    let rootfs_drive = format!("file={},if=none,format=raw,id=hd0", rootfs_path.display());
    let dump_dtb = format!("dumpdtb={}", dtb_path.display());

    base_qemu_cmd(sh, arch, &rootfs_drive)
        .args(["-serial", "stdio", "-machine", &dump_dtb])
        .run()?;

    if !dtb_path.exists() {
        return Err(format!("failed to generate DTB at {}", dtb_path.display()).into());
    }
    inject_firmware_reserved_memory(arch, &dtb_path)?;
    Ok(dtb_path)
}

/// QEMU virt 原生 DTB 不描述第一段 RAM 中的 firmware/bootloader 保留区。
/// 内核需要这个节点来区分固件保留区和普通内核 RAM，因此在打包 FIT 前补充。
fn inject_firmware_reserved_memory(arch: Arch, dtb_path: &Path) -> Result<()> {
    let dts_path = dtb_path.with_extension("dts");
    let status = Command::new("dtc")
        .args(["-q", "-I", "dtb", "-O", "dts", "-o"])
        .arg(&dts_path)
        .arg(dtb_path)
        .status()
        .map_err(|e| format!("dtc 反编译 DTB 失败: {e}"))?;
    if !status.success() {
        return Err(format!("dtc 反编译 DTB 失败: {status}").into());
    }

    let mut dts = fs::read_to_string(&dts_path)?;
    if dts.contains("simplekernel,firmware-reserved") {
        return Ok(());
    }
    if dts.contains("reserved-memory") {
        eprintln!(
            "[xtask] warning: DTB already has /reserved-memory; skip SimpleKernel firmware node injection"
        );
        return Ok(());
    }

    let (addr, size) = match arch {
        Arch::Riscv64 => (0x8000_0000u64, 0x0020_0000u64),
        Arch::Aarch64 => (0x4000_0000u64, 0x0010_0000u64),
    };
    let node = format!(
        "\n\treserved-memory {{\n\t\t#address-cells = <0x02>;\n\t\t#size-cells = <0x02>;\n\t\tranges;\n\n\t\tfirmware@{addr:x} {{\n\t\t\tcompatible = \"simplekernel,firmware-reserved\";\n\t\t\treg = <0x00 0x{addr:08x} 0x00 0x{size:08x}>;\n\t\t\tno-map;\n\t\t}};\n\t}};\n"
    );
    let insert_at = dts
        .find("\n\tmemory@")
        .or_else(|| dts.rfind("\n};"))
        .ok_or("无法定位 DTB root 节点插入点")?;
    dts.insert_str(insert_at, &node);
    fs::write(&dts_path, dts)?;

    let status = Command::new("dtc")
        .args(["-q", "-I", "dts", "-O", "dtb", "-o"])
        .arg(dtb_path)
        .arg(&dts_path)
        .status()
        .map_err(|e| format!("dtc 重新编译 DTB 失败: {e}"))?;
    if !status.success() {
        return Err(format!("dtc 重新编译 DTB 失败: {status}").into());
    }
    println!("[xtask] Injected firmware reserved-memory: addr={addr:#x}, size={size:#x}");
    Ok(())
}

/// 将内核 ELF 和 DTB 打包为 U-Boot FIT 镜像（`boot.its` → `boot.fit`）。
pub fn generate_fit_image(
    arch: Arch,
    sh: &Shell,
    boot_dir: &Path,
    kernel_elf_path: &Path,
    dtb_path: &Path,
) -> Result<PathBuf> {
    println!("[xtask] Generating FIT image...");

    let kernel_abs = fs::canonicalize(kernel_elf_path)?;
    let dtb_abs = fs::canonicalize(dtb_path)?;
    let content = generate_its_content(
        arch,
        &kernel_abs.display().to_string(),
        &dtb_abs.display().to_string(),
    );

    let boot_its = boot_dir.join("boot.its");
    fs::write(&boot_its, content)?;

    let boot_fit = boot_dir.join("boot.fit");
    cmd!(sh, "mkimage -f {boot_its} {boot_fit}").run()?;
    Ok(boot_fit)
}

/// 将 U-Boot 启动脚本编译为 `boot.scr.uimg`。
pub fn generate_boot_script(arch: Arch, sh: &Shell, boot_dir: &Path) -> Result<PathBuf> {
    println!("[xtask] Creating boot script image...");

    let source_script = boot_dir.join("boot_src.txt");
    fs::write(&source_script, arch.boot_script_content())?;

    let boot_scr_uimg = boot_dir.join("boot.scr.uimg");
    cmd!(sh, "mkimage -T script -d {source_script} {boot_scr_uimg}").run()?;
    Ok(boot_scr_uimg)
}

/// 在 `/srv/tftp` 下建立符号链接，供 QEMU 用户网络 TFTP 服务使用。
///
/// 需要对 `/srv/tftp` 有写权限；失败时只打警告，不中断构建流程。
pub fn setup_tftp(boot_dir: &Path) {
    println!("[xtask] Setting up TFTP directory /srv/tftp...");

    if let Err(e) = fs::create_dir_all("/srv/tftp") {
        eprintln!(
            "[xtask] warning: failed to create /srv/tftp: {e}. You may need to run: sudo mkdir -p /srv/tftp"
        );
        return;
    }

    let boot_scr_uimg = boot_dir.join("boot.scr.uimg");
    let _ = fs::remove_file("/srv/tftp/boot.scr.uimg");
    if let Err(e) = std::os::unix::fs::symlink(&boot_scr_uimg, "/srv/tftp/boot.scr.uimg") {
        eprintln!("[xtask] warning: failed to link /srv/tftp/boot.scr.uimg: {e}");
    }

    let _ = fs::remove_file("/srv/tftp/bin");
    if let Err(e) = std::os::unix::fs::symlink(boot_dir, "/srv/tftp/bin") {
        eprintln!("[xtask] warning: failed to link /srv/tftp/bin: {e}");
    }
}

/// QEMU 测试执行结果
pub struct QemuTestResult {
    /// 测试是否成功（exit code == 0）
    pub success: bool,
    /// 是否因超时被终止
    pub timed_out: bool,
    /// 捕获的 stdout + stderr 输出
    pub output: String,
}

/// 启动 QEMU 并捕获输出，带超时。用于自动化测试场景。
///
/// 与 `launch_qemu` 不同，此函数不继承 stdio，而是捕获所有输出并在
/// 超时后自动终止进程。适用于无人值守的测试执行。
pub fn launch_qemu_captured(
    arch: Arch,
    project_root: &Path,
    boot_dir: &Path,
    kernel_elf_path: &Path,
    rootfs_path: &Path,
    timeout_secs: u64,
) -> Result<QemuTestResult> {
    let rootfs_drive = format!("file={},if=none,format=raw,id=hd0", rootfs_path.display());
    let qemu_log = boot_dir.join("qemu.log");
    let fw = arch.firmware_dir(project_root);

    let (machine, cpu) = match arch {
        Arch::Riscv64 => ("virt", "max"),
        Arch::Aarch64 => ("virt,secure=on,gic_version=3", "cortex-a72"),
    };

    let mut cmd = Command::new(arch.qemu_binary());

    // 基础参数（与 base_qemu_cmd 保持一致）
    cmd.args([
        "-nographic",
        "-monitor",
        "none", // 测试模式不需要 monitor，避免端口冲突
        "-m",
        "1024M",
        "-smp",
        "2",
        "-global",
        "virtio-mmio.force-legacy=false",
        "-netdev",
        "user,id=net0,tftp=/srv/tftp,bootfile=boot.scr.uimg",
        "-device",
        "virtio-net-device,netdev=net0",
        "-device",
        "virtio-gpu-device",
        "-drive",
        &rootfs_drive,
        "-device",
        "virtio-blk-device,drive=hd0",
        "-machine",
        machine,
        "-cpu",
        cpu,
    ]);

    // 架构特定参数
    match arch {
        Arch::Riscv64 => {
            let bios = fw.join("u-boot/spl/u-boot-spl.bin");
            let loader = format!(
                "loader,file={},addr=0x80200000",
                fw.join("u-boot/u-boot.itb").display()
            );
            cmd.args(["-serial", "stdio", "-d", "guest_errors,cpu_reset"])
                .arg("-D")
                .arg(&qemu_log)
                .arg("-bios")
                .arg(&bios)
                .arg("-device")
                .arg(&loader);
        }
        Arch::Aarch64 => {
            let bios = fw.join("arm-trusted-firmware/flash.bin");
            let drive = boot_part_drive_arg(boot_dir);
            cmd.args(["-serial", "stdio", "-serial", "null"])
                .args(["-d", "guest_errors,cpu_reset"])
                .arg("-D")
                .arg(&qemu_log)
                .arg("-drive")
                .arg(&drive)
                .arg("-bios")
                .arg(&bios)
                .arg("-kernel")
                .arg(kernel_elf_path);
        }
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("QEMU 启动失败 ({}): {e}", arch.qemu_binary()))?;

    // take stdout/stderr 以便在单独线程中读取，避免管道缓冲区满阻塞
    let child_stdout = child.stdout.take();
    let child_stderr = child.stderr.take();

    let stdout_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut out) = child_stdout {
            let _ = out.read_to_string(&mut buf);
        }
        buf
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut err) = child_stderr {
            let _ = err.read_to_string(&mut buf);
        }
        buf
    });

    // 轮询等待进程结束，超时后 kill
    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();
    let timed_out;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                timed_out = false;
                let stdout_str = stdout_thread.join().unwrap_or_default();
                let stderr_str = stderr_thread.join().unwrap_or_default();
                return Ok(QemuTestResult {
                    success: status.success(),
                    timed_out,
                    output: format!("{stdout_str}{stderr_str}"),
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    let stdout_str = stdout_thread.join().unwrap_or_default();
                    let stderr_str = stderr_thread.join().unwrap_or_default();
                    return Ok(QemuTestResult {
                        success: false,
                        timed_out,
                        output: format!(
                            "{stdout_str}{stderr_str}\n[TIMEOUT after {timeout_secs}s]"
                        ),
                    });
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("等待 QEMU 进程结束失败: {e}").into());
            }
        }
    }
}

/// 准备 aarch64 引导分区目录（`boot/boot_part/`）。
///
/// U-Boot 的 standard boot 通过 `script` bootmeth 在块设备上扫描
/// `boot.scr.uimg`。QEMU 的 `file=fat:rw:dir` 将宿主目录映射为虚拟
/// FAT 块设备。该目录专门存放 U-Boot 启动脚本，与 `boot/` 目录分离，
/// 避免将构建产物（boot.fit、rootfs.img 等大文件）暴露给虚拟 FAT。
pub fn prepare_boot_part(boot_dir: &Path) -> Result<PathBuf> {
    let part_dir = boot_dir.join("boot_part");
    fs::create_dir_all(&part_dir)?;

    let src = boot_dir.join("boot.scr.uimg");
    let dst = part_dir.join("boot.scr.uimg");
    fs::copy(&src, &dst)?;
    Ok(part_dir)
}

/// 返回 aarch64 引导分区的 QEMU 驱动参数。
fn boot_part_drive_arg(boot_dir: &Path) -> String {
    let part_dir = boot_dir.join("boot_part");
    format!("file=fat:rw:{},format=raw,media=disk", part_dir.display())
}

/// 启动 QEMU 运行内核。
///
/// `debug` 为 `true` 时附加 `-s -S`：暂停 CPU 并在 `localhost:1234`
/// 开放 GDB 远程调试端口，等待 GDB 连接后再继续执行。
pub fn launch_qemu(
    sh: &Shell,
    arch: Arch,
    project_root: &Path,
    boot_dir: &Path,
    kernel_elf_path: &Path,
    rootfs_path: &Path,
    debug: bool,
    timeout_secs: Option<u64>,
) -> Result<()> {
    if debug {
        println!(
            "[xtask] Launching QEMU (debug) for {} — attach GDB on port 1234...",
            arch.as_str()
        );
        println!(
            "[xtask] GDB command: gdb-multiarch {} -x {}",
            kernel_elf_path.display(),
            project_root.join("debug.gdb").display()
        );
    } else if let Some(timeout_secs) = timeout_secs {
        println!(
            "[xtask] Launching QEMU for {} (timeout: {}s)...",
            arch.as_str(),
            timeout_secs
        );
    } else {
        println!("[xtask] Launching QEMU for {}...", arch.as_str());
    }

    let rootfs_drive = format!("file={},if=none,format=raw,id=hd0", rootfs_path.display());
    let qemu_log = boot_dir.join("qemu.log");
    let fw = arch.firmware_dir(project_root);

    match arch {
        Arch::Riscv64 => {
            let bios = fw.join("u-boot/spl/u-boot-spl.bin");
            let loader = format!(
                "loader,file={},addr=0x80200000",
                fw.join("u-boot/u-boot.itb").display()
            );
            let mut cmd = match timeout_secs {
                Some(timeout_secs) if !debug => {
                    base_qemu_timeout_cmd(sh, arch, &rootfs_drive, timeout_secs)
                }
                _ => base_qemu_cmd(sh, arch, &rootfs_drive),
            }
            .args(["-serial", "stdio", "-d", "guest_errors,cpu_reset"])
            .arg("-D")
            .arg(&qemu_log)
            .arg("-bios")
            .arg(&bios)
            .arg("-device")
            .arg(&loader);
            if debug {
                cmd = cmd.args(["-s", "-S"]);
            }
            cmd.run()?;
        }
        Arch::Aarch64 => {
            let bios = fw.join("arm-trusted-firmware/flash.bin");
            let drive = boot_part_drive_arg(boot_dir);
            let mut cmd = match timeout_secs {
                Some(timeout_secs) if !debug => {
                    base_qemu_timeout_cmd(sh, arch, &rootfs_drive, timeout_secs)
                }
                _ => base_qemu_cmd(sh, arch, &rootfs_drive),
            }
            .args([
                "-serial",
                "stdio", // 主串口：ATF + U-Boot + 内核 → 直接输出到终端
                "-serial", "null", // OP-TEE 串口：通常无输出，丢弃即可
            ])
            .args(["-d", "guest_errors,cpu_reset"])
            .arg("-D")
            .arg(&qemu_log)
            .arg("-drive")
            .arg(&drive)
            .arg("-bios")
            .arg(&bios)
            .arg("-kernel")
            .arg(kernel_elf_path);
            if debug {
                cmd = cmd.args(["-s", "-S"]);
            }
            cmd.run()?;
        }
    }

    Ok(())
}
