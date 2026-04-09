# P11: 设备文件 + 伪文件系统

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 提供 `/dev/null`、`/dev/zero`、`/dev/console` 设备文件和最小 `/proc` 伪文件系统。BusyBox 启动时 fd 0/1/2 绑定到 `/dev/console`。

**Architecture:** 利用现有 VFS `FileSystem` trait 实现设备文件和 procfs——设备文件作为特殊 inode 由 devfs 管理，procfs 动态生成内容。

**Depends on:** P10（openat/fstat/ioctl syscall 已就绪）

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Create | `src/fs/devfs.rs` | 设备文件系统：/dev/null, /dev/zero, /dev/console |
| Create | `src/fs/procfs.rs` | /proc 伪文件系统 |
| Create | `src/device/chardev.rs` | CharDevice trait + 字符设备注册 |
| Create | `src/device/null.rs` | /dev/null 设备 |
| Create | `src/device/zero.rs` | /dev/zero 设备 |
| Create | `src/device/console.rs` | /dev/console 设备（UART 后端） |
| Create | `src/tty/mod.rs` | TTY 子系统：termios + 行纪律 |
| Modify | `src/fs/mod.rs:fs_init()` | 挂载 devfs + procfs |
| Modify | `src/task/tcb.rs` | 初始化 fd 0/1/2 指向 /dev/console |
| Modify | `src/lib.rs` | 注册 `pub mod tty` |

---

## Task 1: CharDevice trait

**Files:**
- Create: `src/device/chardev.rs`

- [ ] **Step 1: 定义字符设备接口**

```rust
//! 字符设备接口——所有字符设备（null/zero/console/tty）实现此 trait。

/// 字符设备操作。
pub trait CharDevice: Send + Sync {
    /// 设备名称。
    fn name(&self) -> &str;

    /// 从设备读取数据，返回实际读取字节数。
    fn read(&self, buf: &mut [u8]) -> Result<usize, super::DeviceError>;

    /// 向设备写入数据，返回实际写入字节数。
    fn write(&self, data: &[u8]) -> Result<usize, super::DeviceError>;

    /// ioctl 操作。
    fn ioctl(&self, cmd: u32, arg: usize) -> Result<i32, super::DeviceError> {
        let _ = (cmd, arg);
        Err(super::DeviceError::UnsupportedDevice) // ENOTTY
    }
}
```

- [ ] **Step 2: Commit**

---

## Task 2: null / zero / console 设备

**Files:**
- Create: `src/device/null.rs`
- Create: `src/device/zero.rs`
- Create: `src/device/console.rs`

- [ ] **Step 1: /dev/null**

```rust
pub struct NullDevice;

impl CharDevice for NullDevice {
    fn name(&self) -> &str { "null" }
    fn read(&self, _buf: &mut [u8]) -> Result<usize, DeviceError> { Ok(0) } // EOF
    fn write(&self, data: &[u8]) -> Result<usize, DeviceError> { Ok(data.len()) } // discard
}
```

- [ ] **Step 2: /dev/zero**

```rust
pub struct ZeroDevice;

impl CharDevice for ZeroDevice {
    fn name(&self) -> &str { "zero" }
    fn read(&self, buf: &mut [u8]) -> Result<usize, DeviceError> {
        buf.fill(0);
        Ok(buf.len())
    }
    fn write(&self, data: &[u8]) -> Result<usize, DeviceError> { Ok(data.len()) }
}
```

- [ ] **Step 3: /dev/console**

```rust
/// 控制台设备——读写转发到架构 UART。
pub struct ConsoleDevice;

impl CharDevice for ConsoleDevice {
    fn name(&self) -> &str { "console" }

    fn read(&self, buf: &mut [u8]) -> Result<usize, DeviceError> {
        // 从 UART 读取（阻塞式或返回 0）
        // P11: 简单实现——返回 0（不支持输入）
        // TTY 子系统完善后改为行缓冲读取
        Ok(0)
    }

    fn write(&self, data: &[u8]) -> Result<usize, DeviceError> {
        // 转发到 arch console（SBI putchar / PL011）
        for &b in data {
            crate::arch::Arch::console_putchar(b);
        }
        Ok(data.len())
    }

    fn ioctl(&self, cmd: u32, arg: usize) -> Result<i32, DeviceError> {
        match cmd {
            // TCGETS — 返回默认 termios
            0x5401 => {
                let termios = arg as *mut [u8; 60];
                // SAFETY: 调用者保证地址有效
                unsafe { (*termios).fill(0) };
                // 设置 c_lflag: ECHO | ICANON | ISIG
                unsafe {
                    let c_lflag = (arg + 12) as *mut u32;
                    *c_lflag = 0o0_000_013; // ECHO|ICANON|ISIG
                }
                Ok(0)
            }
            // TIOCGWINSZ — 返回 80x24 终端大小
            0x5413 => {
                let winsize = arg as *mut [u16; 4]; // rows, cols, xpixel, ypixel
                unsafe {
                    (*winsize)[0] = 24;  // rows
                    (*winsize)[1] = 80;  // cols
                    (*winsize)[2] = 0;
                    (*winsize)[3] = 0;
                }
                Ok(0)
            }
            _ => Err(DeviceError::UnsupportedDevice),
        }
    }
}
```

- [ ] **Step 4: arch console putchar 接口**

在 `src/arch/mod.rs` 的 `ArchOps` trait 添加：

```rust
fn console_putchar(byte: u8);
```

RISC-V 实现：SBI `console_putchar` ecall。
AArch64 实现：PL011 UART 写。

- [ ] **Step 5: Commit**

---

## Task 3: 设备文件系统（devfs）

**Files:**
- Create: `src/fs/devfs.rs`

- [ ] **Step 1: 实现 devfs**

devfs 实现 `FileSystem` trait，将设备节点暴露为文件。

```rust
//! 设备文件系统——将字符设备暴露为 /dev/xxx 文件节点。

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::string::String;

use super::vfs::*;
use crate::device::chardev::CharDevice;

/// devfs 中的设备节点。
struct DevNode {
    name: String,
    device: Arc<dyn CharDevice>,
    inode: InodeId,
}

/// 设备文件系统。
pub struct DevFs {
    nodes: sync::SpinLock<BTreeMap<InodeId, DevNode>>,
    root: InodeId,
    names: sync::SpinLock<BTreeMap<String, InodeId>>,
    next_id: core::sync::atomic::AtomicU64,
}

impl DevFs {
    pub fn new() -> Self { /* ... */ }

    /// 注册字符设备。
    pub fn register(&self, name: &str, device: Arc<dyn CharDevice>) -> InodeId {
        let id = InodeId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let node = DevNode {
            name: String::from(name),
            device,
            inode: id,
        };
        self.nodes.lock().insert(id, node);
        self.names.lock().insert(String::from(name), id);
        id
    }
}

impl FileSystem for DevFs {
    fn name(&self) -> &str { "devfs" }
    fn root_inode(&self) -> InodeId { self.root }

    fn lookup(&self, parent: InodeId, name: &str) -> FsResult<Option<InodeId>> {
        if parent != self.root { return Err(FsError::NotADirectory); }
        Ok(self.names.lock().get(name).copied())
    }

    fn read(&self, inode: InodeId, _offset: u64, buf: &mut [u8]) -> FsResult<usize> {
        let nodes = self.nodes.lock();
        let node = nodes.get(&inode).ok_or(FsError::NotFound)?;
        node.device.read(buf).map_err(|_| FsError::IoError)
    }

    fn write(&self, inode: InodeId, _offset: u64, data: &[u8]) -> FsResult<usize> {
        let nodes = self.nodes.lock();
        let node = nodes.get(&inode).ok_or(FsError::NotFound)?;
        node.device.write(data).map_err(|_| FsError::IoError)
    }

    fn readdir(&self, inode: InodeId) -> FsResult<alloc::vec::Vec<DirEntry>> {
        if inode != self.root { return Err(FsError::NotADirectory); }
        let nodes = self.nodes.lock();
        Ok(nodes.values().map(|n| DirEntry {
            name: n.name.clone(),
            inode: n.inode,
            file_type: FileType::Regular, // 设备文件显示为 regular
        }).collect())
    }

    // create/mkdir/unlink/stat: 基本实现或 NotSupported
    fn create(&self, _p: InodeId, _n: &str, _t: FileType) -> FsResult<InodeId> {
        Err(FsError::NotSupported)
    }
    fn mkdir(&self, _p: InodeId, _n: &str) -> FsResult<InodeId> {
        Err(FsError::NotSupported)
    }
    fn unlink(&self, _p: InodeId, _n: &str) -> FsResult<()> {
        Err(FsError::NotSupported)
    }
    fn stat(&self, inode: InodeId) -> FsResult<InodeStat> {
        let nodes = self.nodes.lock();
        let _node = nodes.get(&inode).ok_or(FsError::NotFound)?;
        Ok(InodeStat { size: 0, file_type: FileType::Regular, permissions: 0o666 })
    }
}
```

- [ ] **Step 2: 在 `fs_init()` 中挂载 devfs 并注册设备**

```rust
// 在 fs_init() 中，挂载 RamFS 后添加：
let devfs = Arc::new(devfs::DevFs::new());
devfs.register("null", Arc::new(crate::device::null::NullDevice));
devfs.register("zero", Arc::new(crate::device::zero::ZeroDevice));
devfs.register("console", Arc::new(crate::device::console::ConsoleDevice));
mount("/dev", devfs);
```

- [ ] **Step 3: Commit**

---

## Task 4: fd 0/1/2 绑定到 /dev/console

**Files:**
- Modify: `src/fs/fd_table.rs`
- Modify: `src/task/tcb.rs`

- [ ] **Step 1: `FileDescriptorTable` 添加 `init_stdio` 方法**

```rust
/// 初始化 fd 0/1/2 指向 /dev/console。
pub fn init_stdio(console_fs: Arc<dyn FileSystem>, console_inode: InodeId) -> Self {
    let mut table = Self::new();
    for fd_num in 0..3 {
        let file = File {
            fs: console_fs.clone(),
            inode: console_inode,
            offset: 0,
            flags: OpenFlags::RDWR,
        };
        let wrapped = Arc::new(SpinLock::new(file, "stdio_fd", sync::lock_level::UNSPECIFIED));
        table.entries[fd_num] = Some(wrapped);
    }
    table
}
```

- [ ] **Step 2: 用户进程创建时使用 `init_stdio`**

在 execve 或 fork 创建用户进程时，FD 表用 `init_stdio` 初始化（而非空 `new()`）。

- [ ] **Step 3: Commit**

---

## Task 5: procfs（最小集）

**Files:**
- Create: `src/fs/procfs.rs`

- [ ] **Step 1: 实现 procfs**

BusyBox 最常访问的 `/proc` 路径：
- `/proc/self/exe` → readlink 返回可执行文件路径
- `/proc/mounts` → 已挂载文件系统列表
- `/proc/version` → 内核版本字符串

```rust
pub struct ProcFs {
    root: InodeId,
}

impl FileSystem for ProcFs {
    fn name(&self) -> &str { "proc" }

    fn read(&self, inode: InodeId, offset: u64, buf: &mut [u8]) -> FsResult<usize> {
        // 根据 inode 动态生成内容
        let content = match inode.0 {
            100 => self.generate_mounts(),
            101 => alloc::format!("SimpleKernel version 0.1.0\n"),
            _ => return Err(FsError::NotFound),
        };
        let bytes = content.as_bytes();
        let start = offset as usize;
        if start >= bytes.len() { return Ok(0); }
        let n = buf.len().min(bytes.len() - start);
        buf[..n].copy_from_slice(&bytes[start..start + n]);
        Ok(n)
    }

    fn lookup(&self, parent: InodeId, name: &str) -> FsResult<Option<InodeId>> {
        if parent != self.root { return Err(FsError::NotADirectory); }
        match name {
            "mounts" => Ok(Some(InodeId(100))),
            "version" => Ok(Some(InodeId(101))),
            "self" => Ok(Some(InodeId(200))), // /proc/self 目录
            _ => Ok(None),
        }
    }

    // ... 其他 FileSystem 方法
}
```

- [ ] **Step 2: 挂载到 /proc**
- [ ] **Step 3: Commit**

---

## Task 6: ioctl 设备分发

- [ ] **Step 1: sys_ioctl 增强**

```rust
pub fn sys_ioctl(fd: u32, cmd: u32, arg: usize) -> i64 {
    let task = crate::task::current_task();
    let file_ref = task.fd_table().lock().get(Fd(fd));
    let file = file_ref.ok_or(-9i64)?; // EBADF
    let file_guard = file.lock();

    // 检查是否为 devfs 设备文件
    if let Some(devfs) = file_guard.fs.as_any().downcast_ref::<DevFs>() {
        // 查找设备并转发 ioctl
        return devfs.ioctl(file_guard.inode, cmd, arg)
            .map(|r| r as i64)
            .unwrap_or(-25); // ENOTTY
    }

    -25 // ENOTTY
}
```

**注意**：`FileSystem` trait 需要添加 `as_any()` 方法以支持 downcast：

```rust
fn as_any(&self) -> &dyn core::any::Any;
```

或者在 `FileSystem` trait 中直接添加 `ioctl` 方法：

```rust
fn ioctl(&self, _inode: InodeId, _cmd: u32, _arg: usize) -> FsResult<i32> {
    Err(FsError::NotSupported)
}
```

后者更简洁，推荐。

- [ ] **Step 2: Commit**

---

## Task 7: TTY 子系统基础（可延后）

如果 BusyBox sh 需要行编辑功能，则需要基本的 TTY 行纪律：

```rust
//! TTY 行纪律——处理行缓冲、回显、特殊按键。

pub struct LineDiscipline {
    /// 行缓冲区
    line_buf: [u8; 256],
    line_len: usize,
    /// 是否启用回显
    echo: bool,
    /// 是否启用 canonical 模式（行缓冲）
    canonical: bool,
}
```

P11 可先跳过，在 P12 调试时按需实现。
