# P12: BusyBox 集成

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** SimpleKernel 启动后加载 BusyBox 作为 `/init`，运行交互式 `sh`，支持基本命令（`ls`、`cat`、`echo`、`mkdir`、`rm`、`ps`）。

**Architecture:** BusyBox 交叉编译为 riscv64/aarch64 静态链接 musl 二进制。打包到 initramfs（CPIO 格式）或 FAT 磁盘镜像。内核启动后 execve("/init")。

**Depends on:** P8-P11 全部完成

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Create | `tools/busybox/` | BusyBox 构建脚本 + 配置 |
| Create | `tools/busybox/build.sh` | 交叉编译 BusyBox |
| Create | `tools/busybox/busybox_config` | BusyBox .config（最小集） |
| Create | `tools/initramfs/` | initramfs 构建工具 |
| Create | `tools/initramfs/build.sh` | 打包 CPIO initramfs |
| Create | `tools/initramfs/init` | /init 脚本 |
| Create | `src/initramfs.rs` | initramfs 解析（CPIO newc 格式） |
| Modify | `src/boot.rs` | 加载 initramfs → execve /init |
| Modify | `src/main.rs` | bootstrap 启动 init 进程 |
| Create | `tests/umode-test/src/busybox_boot.rs` | BusyBox 启动测试 |

---

## Task 1: 交叉编译 BusyBox

- [ ] **Step 1: 创建构建脚本**

`tools/busybox/build.sh`:

```bash
#!/bin/bash
set -euo pipefail

ARCH=${1:-riscv64}
BUSYBOX_VERSION=1.36.1
BUSYBOX_DIR="busybox-${BUSYBOX_VERSION}"
BUSYBOX_TAR="${BUSYBOX_DIR}.tar.bz2"

# 下载
if [ ! -f "${BUSYBOX_TAR}" ]; then
    wget "https://busybox.net/downloads/${BUSYBOX_TAR}"
fi
if [ ! -d "${BUSYBOX_DIR}" ]; then
    tar xjf "${BUSYBOX_TAR}"
fi

cd "${BUSYBOX_DIR}"

# 设置交叉编译工具链
case "${ARCH}" in
    riscv64)
        export CROSS_COMPILE=riscv64-linux-gnu-
        export ARCH=riscv
        ;;
    aarch64)
        export CROSS_COMPILE=aarch64-linux-gnu-
        export ARCH=arm64
        ;;
esac

# 使用预配置
if [ -f ../busybox_config ]; then
    cp ../busybox_config .config
else
    make defconfig
fi

# 关键配置：静态链接
sed -i 's/# CONFIG_STATIC is not set/CONFIG_STATIC=y/' .config

# 编译
make -j$(nproc)

echo "BusyBox built: ${BUSYBOX_DIR}/busybox"
```

- [ ] **Step 2: 创建最小配置**

`tools/busybox/busybox_config` — 仅启用 BusyBox sh 所需的最小命令集：

```
CONFIG_STATIC=y
CONFIG_SH_IS_ASH=y
CONFIG_ASH=y
CONFIG_ASH_ECHO=y
CONFIG_LS=y
CONFIG_CAT=y
CONFIG_ECHO=y
CONFIG_MKDIR=y
CONFIG_RM=y
CONFIG_CP=y
CONFIG_MV=y
CONFIG_PS=y
CONFIG_MOUNT=y
CONFIG_UMOUNT=y
CONFIG_INIT=y
# 禁用不需要的功能以减小体积
CONFIG_FEATURE_SH_IS_NONE=n
CONFIG_FEATURE_EDITING=y
CONFIG_FEATURE_TAB_COMPLETION=y
```

- [ ] **Step 3: 构建并验证**

```bash
cd tools/busybox
bash build.sh riscv64
file busybox-1.36.1/busybox
# 应输出: ELF 64-bit LSB executable, UCB RISC-V, statically linked
```

- [ ] **Step 4: Commit**

---

## Task 2: 构建 initramfs

- [ ] **Step 1: 创建 init 脚本**

`tools/initramfs/init`:

```bash
#!/bin/sh
# SimpleKernel init — BusyBox 启动脚本

# 挂载伪文件系统
/bin/mount -t proc proc /proc
/bin/mount -t devtmpfs devtmpfs /dev

echo "SimpleKernel booted successfully"
echo "Welcome to BusyBox shell"

# 启动交互式 shell
exec /bin/sh
```

- [ ] **Step 2: 打包 CPIO**

`tools/initramfs/build.sh`:

```bash
#!/bin/bash
set -euo pipefail

ARCH=${1:-riscv64}
ROOTFS=rootfs
BUSYBOX=../busybox/busybox-1.36.1/busybox

rm -rf "${ROOTFS}"
mkdir -p "${ROOTFS}"/{bin,sbin,dev,proc,sys,tmp,etc}

# 安装 BusyBox
cp "${BUSYBOX}" "${ROOTFS}/bin/busybox"
chmod +x "${ROOTFS}/bin/busybox"

# 创建符号链接
cd "${ROOTFS}/bin"
for cmd in sh ls cat echo mkdir rm cp mv ps mount umount; do
    ln -sf busybox "${cmd}"
done
cd -

# 安装 init
cp init "${ROOTFS}/init"
chmod +x "${ROOTFS}/init"

# 创建设备节点（供内核自动使用）
# BusyBox init 通常会自己创建这些，但预创建更安全
sudo mknod "${ROOTFS}/dev/console" c 5 1 2>/dev/null || true
sudo mknod "${ROOTFS}/dev/null" c 1 3 2>/dev/null || true

# 打包 CPIO (newc 格式)
cd "${ROOTFS}"
find . | cpio -o -H newc > ../initramfs.cpio
cd -

echo "initramfs built: tools/initramfs/initramfs.cpio"
echo "Size: $(du -h initramfs.cpio | cut -f1)"
```

- [ ] **Step 3: Commit**

---

## Task 3: 内核 initramfs 加载

**两种加载方式，二选一：**

### 方式 A: QEMU -initrd 参数（推荐）

QEMU 将 initramfs 放到内存中的指定地址，通过 FDT `/chosen/linux,initrd-start` 和 `/chosen/linux,initrd-end` 告知内核。

- [ ] **Step 1: 从 FDT 获取 initramfs 地址**

在 `src/fdt.rs` 的 `KernelFdt` 中添加：

```rust
/// 获取 initramfs 的内存范围。
pub fn initrd(&self) -> Option<(usize, usize)> {
    let chosen = self.fdt.find_node("/chosen")?;
    let start = chosen.property("linux,initrd-start")?
        .as_usize()?;
    let end = chosen.property("linux,initrd-end")?
        .as_usize()?;
    Some((start, end))
}
```

### 方式 B: FAT 磁盘镜像

将 initramfs 文件放到 FAT 文件系统中，内核从 `/initramfs.cpio` 读取。

---

- [ ] **Step 2: 创建 `src/initramfs.rs`**

CPIO newc 格式解析：

```rust
//! CPIO newc 格式解析——从 initramfs 提取文件到 RamFS。

/// CPIO newc header（110 字节 ASCII）
/// "070701" magic + 字段均为 8 位十六进制 ASCII
struct CpioEntry<'a> {
    name: &'a str,
    data: &'a [u8],
    mode: u32,
    is_dir: bool,
}

/// 解析 initramfs 并将所有文件写入根文件系统。
pub fn extract_initramfs(data: &[u8]) {
    let mut offset = 0;
    while offset < data.len() {
        let entry = match parse_cpio_entry(&data[offset..]) {
            Some((e, next_offset)) => { offset += next_offset; e }
            None => break,
        };

        if entry.name == "TRAILER!!!" {
            break;
        }
        if entry.name == "." || entry.name.is_empty() {
            continue;
        }

        let path = alloc::format!("/{}", entry.name);
        if entry.is_dir {
            let _ = crate::fs::create_file(&path, crate::fs::vfs::FileType::Directory);
        } else {
            match crate::fs::create_file(&path, crate::fs::vfs::FileType::Regular) {
                Ok((fs, inode)) => {
                    fs.write(inode, 0, entry.data)
                        .expect("initramfs: 写入文件失败");
                }
                Err(e) => log::warn!("initramfs: 创建 {} 失败: {:?}", path, e),
            }
        }
        log::info!("initramfs: {} ({} bytes)", path, entry.data.len());
    }
}

fn parse_cpio_entry(data: &[u8]) -> Option<(CpioEntry, usize)> {
    if data.len() < 110 { return None; }
    // 验证 magic "070701"
    if &data[0..6] != b"070701" { return None; }
    // 解析字段（8 位十六进制 ASCII）
    let namesize = parse_hex(&data[94..102])?;
    let filesize = parse_hex(&data[54..62])?;
    let mode = parse_hex(&data[14..22])?;
    let name_start = 110;
    let name_end = name_start + namesize - 1; // -1 去掉 null terminator
    let name = core::str::from_utf8(&data[name_start..name_end]).ok()?;
    // CPIO 对齐到 4 字节
    let data_start = align4(name_start + namesize);
    let data_end = data_start + filesize;
    let next = align4(data_end);

    Some((CpioEntry {
        name,
        data: &data[data_start..data_end],
        mode: mode as u32,
        is_dir: (mode & 0o040000) != 0,
    }, next))
}

fn parse_hex(ascii: &[u8]) -> Option<usize> {
    let s = core::str::from_utf8(ascii).ok()?;
    usize::from_str_radix(s, 16).ok()
}

fn align4(x: usize) -> usize { (x + 3) & !3 }
```

- [ ] **Step 3: Commit**

---

## Task 4: 启动 init 进程

- [ ] **Step 1: 修改 `bootstrap()` 启动 init**

在 `src/main.rs` 的 `bootstrap()` 中，`smoke_test::spawn_all()` 之后添加：

```rust
// 提取 initramfs（如果存在）
if let Some((start, end)) = crate::fdt::KernelFdt::new(fdt_addr)
    .ok()
    .and_then(|fdt| fdt.initrd())
{
    let data = unsafe { core::slice::from_raw_parts(start as *const u8, end - start) };
    crate::initramfs::extract_initramfs(data);
    log::info!("initramfs extracted");
}

// 启动 init 进程
task::spawn_kernel_thread("init", init_thread, 0);
```

```rust
/// init 内核线程——execve /init 切换到用户态。
fn init_thread(_arg: usize) {
    log::info!("init: execve /init");
    crate::syscall::exec::sys_execve_internal(
        "/init",
        &["/init"],
        &["HOME=/", "PATH=/bin:/sbin"],
    );
}
```

- [ ] **Step 2: xtask 添加 -initrd 参数**

修改 `xtask/src/run.rs`（或对应文件），在 QEMU 启动命令中添加：

```
-initrd path/to/initramfs.cpio
```

- [ ] **Step 3: Commit**

---

## Task 5: 迭代调试

BusyBox 启动时会触发大量 syscall。使用以下流程迭代：

- [ ] **Step 1: 运行并收集缺失 syscall 日志**

```bash
cargo xtask run --arch riscv64 2>&1 | grep "未实现 syscall"
```

收集输出，按出现频率排序。

- [ ] **Step 2: 逐个实现缺失 syscall**

对于每个缺失的 syscall：
1. 查阅 Linux man page 确认参数和返回值
2. 判断是否需要完整实现还是 stub 即可
3. 实现并注册到 dispatch
4. 重新运行测试

**常见的 stub-able syscall：**
- `prlimit64` → 返回 0
- `getrandom` → 填充伪随机数据
- `sched_getaffinity` → 返回单核掩码
- `set_robust_list` → 返回 0
- `rseq` → 返回 -ENOSYS

- [ ] **Step 3: 处理 musl libc 特有的启动序列**

musl 静态链接二进制的启动流程：
```
_start → __libc_start_main → __init_tls → __init_tp
       → set_tid_address → mmap(TLS区域) → mprotect(guard page)
       → rt_sigaction(SIGSEGV/SIGBUS/...)
       → main()
```

关键依赖：
- `set_tid_address`（必须返回有效 PID）
- `mmap`（TLS 区域分配，通常 ~4KB）
- `mprotect`（guard page）
- `rt_sigaction`（注册默认信号处理）

- [ ] **Step 4: BusyBox sh 交互测试**

成功启动 sh 后验证：
```
# echo hello
hello
# ls /
bin  dev  init  proc  sys  tmp
# cat /proc/version
SimpleKernel version 0.1.0
# mkdir /tmp/test
# echo abc > /tmp/test/file.txt
# cat /tmp/test/file.txt
abc
```

- [ ] **Step 5: Commit**

---

## Task 6: QEMU 自动化测试

- [ ] **Step 1: 创建测试二进制 `busybox_boot.rs`**

```rust
test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test);

fn run_test() {
    // 1. 提取 initramfs
    // 2. execve /init
    // 3. 等待输出 "Welcome to BusyBox shell"
    // 4. 如果出现则 PASSED，超时则 FAILED
}
```

- [ ] **Step 2: 集成到 `cargo xtask test`**
- [ ] **Step 3: Commit**

---

## 里程碑验收标准

| 里程碑 | 验收条件 |
|--------|---------|
| M1: 启动 | QEMU 输出 `Welcome to BusyBox shell`，出现 `#` 提示符 |
| M2: echo | `echo hello` 输出 `hello` |
| M3: ls | `ls /` 显示正确的目录内容 |
| M4: cat | `cat /proc/version` 输出内核版本 |
| M5: 管道 | `echo hello | cat` 正常工作 |
| M6: 文件操作 | `mkdir /tmp/x && echo test > /tmp/x/f && cat /tmp/x/f` 输出 `test` |

---

## 预计工作量

| 阶段 | 新增代码量 | 核心难度 |
|------|-----------|---------|
| P8: ecall 基础 | ~300 行 | 低（基础设施已完备） |
| P9: ELF + 进程 | ~3000 行 | 高（per-process 页表、fork） |
| P10: 50+ syscall | ~4000 行 | 中（量大但单个简单） |
| P11: 设备 + 伪 FS | ~2000 行 | 中 |
| P12: BusyBox 集成 | ~1500 行 + 调试 | 中（主要是调试补漏） |
| **合计** | **~10,000-12,000 行** | |

当前代码：12,600 行 → 完成后约 23,000-25,000 行。
