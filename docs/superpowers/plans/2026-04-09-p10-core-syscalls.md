# P10: 核心 Syscall 扩充

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 BusyBox `sh` 启动所需的最小 syscall 集（~50 个），覆盖进程、内存、文件、信号、系统信息五个类别。

**Architecture:** 在 P8 的 `abi::dispatch` 基础上扩展 match 分支。每个 syscall 分为两层：`abi.rs` 负责参数解码（u64→Rust 类型）+ 调用内部实现。实现层复用现有子系统（task/memory/fs/signal）。

**Depends on:** P8（dispatch 基础设施）、P9（execve/fork/地址空间）

---

## Syscall 清单（按优先级排序）

### Tier 1: musl libc 启动必需（不实现则 `_start` 都过不去）

| NR | Name | 参数 | 实现难度 | 说明 |
|----|------|------|---------|------|
| 93 | `exit` | code | ✅ P8已有 | |
| 94 | `exit_group` | code | 低 | 同 exit（单线程） |
| 96 | `set_tid_address` | tidptr | 低 | 保存指针到 TCB，返回 PID |
| 214 | `brk` | addr | 中 | 扩展/收缩 user heap |
| 222 | `mmap` | addr,len,prot,flags,fd,off | 高 | 匿名映射 + MAP_FIXED |
| 215 | `munmap` | addr, len | 中 | 释放映射 |
| 226 | `mprotect` | addr, len, prot | 中 | 修改权限 |
| 134 | `rt_sigaction` | sig, act, oact | 中 | 注册信号处理函数 |
| 135 | `rt_sigprocmask` | how, set, oset | 低 | 设置信号掩码 |
| 160 | `uname` | buf | 低 | 返回系统名称 |
| 64 | `write` | fd,buf,len | ✅ P8已有 | |
| 63 | `read` | fd,buf,len | 中 | |
| 57 | `close` | fd | ✅ 已有 | |

### Tier 2: BusyBox sh 基本交互

| NR | Name | 参数 | 实现难度 | 说明 |
|----|------|------|---------|------|
| 56 | `openat` | dirfd,path,flags,mode | 中 | AT_FDCWD 支持 |
| 48 | `faccessat` | dirfd,path,mode,flags | 低 | 文件存在性检查 |
| 79 | `fstatat`/`newfstatat` | dirfd,path,statbuf,flags | 中 | struct stat 构造 |
| 80 | `fstat` | fd, statbuf | 中 | |
| 61 | `getdents64` | fd, dirp, count | 中 | 目录遍历（ls 必需） |
| 17 | `getcwd` | buf, size | 低 | 当前目录 |
| 49 | `chdir` | path | 低 | 切换目录 |
| 23 | `dup` | oldfd | 低 | |
| 24 | `dup3` | oldfd, newfd, flags | 低 | |
| 59 | `pipe2` | pipefd, flags | 中 | shell 管道 |
| 25 | `fcntl` | fd, cmd, arg | 低 | F_DUPFD, F_GETFD, F_SETFD, F_GETFL, F_SETFL |
| 29 | `ioctl` | fd, cmd, arg | 中 | TCGETS/TCSETS/TIOCGWINSZ（终端） |
| 66 | `writev` | fd, iov, iovcnt | 低 | gather write |
| 172 | `getpid` | — | 低 | |
| 173 | `getppid` | — | 低 | |
| 174-177 | `getuid/getgid/geteuid/getegid` | — | 低 | stub 返回 0 |
| 113 | `clock_gettime` | clk_id, tp | 低 | CLOCK_MONOTONIC |
| 169 | `gettimeofday` | tv, tz | 低 | |

### Tier 3: BusyBox 完整功能

| NR | Name | 说明 |
|----|------|------|
| 35 | `unlinkat` | 删除文件 |
| 34 | `mkdirat` | 创建目录 |
| 78 | `readlinkat` | 读取符号链接 |
| 276 | `renameat2` | 重命名 |
| 62 | `lseek` | 文件偏移（已有内部实现） |
| 46 | `ftruncate` | 截断文件 |
| 73 | `ppoll` | I/O 多路复用 |
| 139 | `rt_sigreturn` | 信号返回 |
| 260 | `wait4` | 等待子进程（扩展现有 waitpid） |
| 261 | `prlimit64` | 资源限制（stub） |
| 278 | `getrandom` | 随机数（stub） |

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Modify | `src/syscall/abi.rs` | dispatch 扩展到 ~50 个 syscall |
| Create | `src/syscall/memory.rs` | brk, mmap, munmap, mprotect |
| Create | `src/syscall/signal.rs` | rt_sigaction, rt_sigprocmask, rt_sigreturn |
| Create | `src/syscall/info.rs` | uname, getpid, getppid, getuid, clock_gettime |
| Create | `src/syscall/dir.rs` | getcwd, chdir, getdents64 |
| Modify | `src/syscall/file.rs` | openat, fstat, dup, pipe2, fcntl, ioctl, writev |
| Modify | `src/syscall/process.rs` | wait4, set_tid_address, exit_group |
| Modify | `src/syscall/mod.rs` | 注册新模块 |
| Create | `src/task/cwd.rs` | per-task 当前工作目录 |
| Modify | `src/task/tcb.rs` | 添加 cwd / tid_address 字段 |
| Create | `src/task/signal_handler.rs` | sigaction 表 + 信号帧构造 |
| Create | `src/fs/pipe.rs` | pipe 实现 |

---

## Task 1: 进程信息类 syscall（最简单，先热身）

- [ ] **Step 1: 创建 `src/syscall/info.rs`**

```rust
//! 系统信息 syscall——getpid, getppid, uname, clock_gettime 等。

/// sys_getpid — 返回当前任务 PID。
pub fn sys_getpid() -> i64 {
    crate::task::current_task().pid() as i64
}

/// sys_getppid — 返回父任务 PID。
pub fn sys_getppid() -> i64 {
    crate::task::current_task()
        .parent_pid()
        .unwrap_or(0) as i64
}

/// sys_getuid / getgid / geteuid / getegid — stub 返回 0（root）。
pub fn sys_getuid() -> i64 { 0 }
pub fn sys_getgid() -> i64 { 0 }

/// sys_set_tid_address — 保存 tidptr 到 TCB，返回 PID。
///
/// musl libc __init_tls 调用此 syscall。
pub fn sys_set_tid_address(tidptr: usize) -> i64 {
    let task = crate::task::current_task();
    task.set_tid_address(tidptr);
    task.pid() as i64
}

/// sys_uname — 填充 `struct utsname`。
///
/// ```c
/// struct utsname {
///     char sysname[65];
///     char nodename[65];
///     char release[65];
///     char version[65];
///     char machine[65];
///     char domainname[65];
/// };
/// ```
pub fn sys_uname(buf_addr: usize) -> i64 {
    let buf = buf_addr as *mut [u8; 65 * 6];
    // SAFETY: 调用者保证 buf_addr 指向有效的用户内存
    let buf = unsafe { &mut *buf };
    buf.fill(0);

    fn write_field(buf: &mut [u8; 65 * 6], index: usize, value: &[u8]) {
        let start = index * 65;
        let len = value.len().min(64);
        buf[start..start + len].copy_from_slice(&value[..len]);
    }

    write_field(buf, 0, b"SimpleKernel");  // sysname
    write_field(buf, 1, b"localhost");      // nodename
    write_field(buf, 2, b"0.1.0");         // release
    write_field(buf, 3, b"SAS-Linux-Compat"); // version
    #[cfg(target_arch = "riscv64")]
    write_field(buf, 4, b"riscv64");       // machine
    #[cfg(target_arch = "aarch64")]
    write_field(buf, 4, b"aarch64");       // machine
    write_field(buf, 5, b"");              // domainname

    0
}

/// sys_clock_gettime — 返回时钟时间。
pub fn sys_clock_gettime(clk_id: u32, tp_addr: usize) -> i64 {
    let ticks = global_tick::current();
    let freq = config::TIMER_FREQ_HZ;
    let sec = ticks / freq;
    let nsec = (ticks % freq) * 1_000_000_000 / freq;

    // struct timespec { time_t tv_sec; long tv_nsec; }
    let tp = tp_addr as *mut [u64; 2];
    // SAFETY: 调用者保证 tp_addr 有效
    unsafe {
        (*tp)[0] = sec;
        (*tp)[1] = nsec;
    }
    0
}
```

- [ ] **Step 2: 在 abi.rs 注册**
- [ ] **Step 3: Commit**

---

## Task 2: 内存管理 syscall（brk, mmap, munmap, mprotect）

- [ ] **Step 1: 创建 `src/syscall/memory.rs`**

关键实现：
- `sys_brk(addr)`: 调用 `UserAddressSpace::brk()`，扩展/收缩堆区域
- `sys_mmap(addr, len, prot, flags, fd, offset)`:
  - `MAP_ANONYMOUS | MAP_PRIVATE`: 分配零化帧，映射到用户地址空间
  - `MAP_FIXED`: 在指定地址映射（先 unmap 旧映射）
  - 文件映射：暂返回 `-ENOSYS`（P10+ 实现）
- `sys_munmap(addr, len)`: 释放用户 VMA + 帧
- `sys_mprotect(addr, len, prot)`: 更新 PTE flags

- [ ] **Step 2: prot → PteFlags 转换**

```rust
fn prot_to_flags(prot: u32) -> PteFlags {
    const PROT_READ: u32 = 1;
    const PROT_WRITE: u32 = 2;
    const PROT_EXEC: u32 = 4;
    let r = prot & PROT_READ != 0;
    let w = prot & PROT_WRITE != 0;
    let x = prot & PROT_EXEC != 0;
    match (r, w, x) {
        (_, true, true)  => PteFlags::user_rwx(),
        (_, true, false) => PteFlags::user_rw(),
        (true, false, true)  => PteFlags::user_rx(),
        _ => PteFlags::user_ro(),
    }
}
```

- [ ] **Step 3: 注册 + 测试**
- [ ] **Step 4: Commit**

---

## Task 3: 文件 syscall 扩展

- [ ] **Step 1: openat（AT_FDCWD 支持）**

`openat(dirfd, path, flags, mode)` — 当 `dirfd == AT_FDCWD(-100)` 时使用当前工作目录。初始实现：直接调用现有 `resolve_path`（假设绝对路径）。

- [ ] **Step 2: fstat / fstatat**

构造 Linux `struct stat`（共 128 字节）。从 VFS `InodeStat` 转换：

```rust
/// Linux struct stat (riscv64/aarch64) 主要字段偏移：
/// st_dev(0), st_ino(8), st_mode(16), st_nlink(24), st_uid(28), st_gid(32),
/// st_rdev(40), _pad(48), st_size(56), st_blksize(64), _pad2(68),
/// st_blocks(72), st_atim(80), st_mtim(96), st_ctim(112)
fn fill_stat(buf_addr: usize, stat: &InodeStat, inode_id: InodeId) {
    let buf = buf_addr as *mut [u8; 128];
    let buf = unsafe { &mut *buf };
    buf.fill(0);
    // st_ino
    buf[8..16].copy_from_slice(&(inode_id.0).to_le_bytes());
    // st_mode
    let mode: u32 = stat.permissions | match stat.file_type {
        FileType::Regular => 0o100000,
        FileType::Directory => 0o040000,
        FileType::SymLink => 0o120000,
    };
    buf[16..20].copy_from_slice(&mode.to_le_bytes());
    // st_nlink
    buf[24..28].copy_from_slice(&1u32.to_le_bytes());
    // st_size
    buf[56..64].copy_from_slice(&stat.size.to_le_bytes());
    // st_blksize
    buf[64..68].copy_from_slice(&512u32.to_le_bytes());
}
```

- [ ] **Step 3: dup / dup3**

```rust
pub fn sys_dup(oldfd: u32) -> i64 {
    let task = crate::task::current_task();
    let mut table = task.fd_table().lock();
    let file = table.get(Fd(oldfd)).ok_or(-9i64 /* EBADF */)?;
    // 分配新 fd，共享同一 Arc<SpinLock<File>>
    // ... FdTable 需要添加 dup() 方法
}
```

- [ ] **Step 4: pipe2**

创建 `src/fs/pipe.rs`：

```rust
/// Pipe 实现——环形缓冲区 + 读写端 FileSystem 适配。
///
/// pipe 不是文件系统，但实现 FileSystem trait 复用 VFS 路径。
/// 读端 inode=0，写端 inode=1。
pub struct Pipe {
    buf: SpinLock<PipeBuffer>,
}

struct PipeBuffer {
    data: [u8; PIPE_BUF_SIZE],
    read_pos: usize,
    write_pos: usize,
    /// 写端是否已关闭
    write_closed: bool,
}
```

- [ ] **Step 5: getdents64**

从 VFS `readdir` 结果构造 `struct linux_dirent64`：

```rust
/// struct linux_dirent64 {
///     d_ino: u64,
///     d_off: i64,
///     d_reclen: u16,
///     d_type: u8,
///     d_name: [u8],  // 可变长，null 结尾
/// }
```

- [ ] **Step 6: fcntl（最小集）**

F_DUPFD, F_GETFD, F_SETFD(CLOEXEC), F_GETFL, F_SETFL。

- [ ] **Step 7: ioctl（终端基础）**

暂时对 fd=0/1/2 返回假数据：
- TCGETS(0x5401): 返回默认 termios
- TIOCGWINSZ(0x5413): 返回 80x24
- 其他: `-ENOTTY`

- [ ] **Step 8: writev**

```rust
pub fn sys_writev(fd: u32, iov_addr: usize, iovcnt: usize) -> i64 {
    // 遍历 iov 数组，对每个 {base, len} 调用 sys_write
}
```

- [ ] **Step 9: 注册全部 + Commit**

---

## Task 4: 信号 syscall

- [ ] **Step 1: 创建 `src/task/signal_handler.rs`**

```rust
/// Per-process 信号处理函数表。
///
/// 每个信号可以设置为：Default / Ignore / UserHandler(fn_addr)
pub struct SigActionTable {
    actions: [SigAction; 32],
}

pub enum SigAction {
    Default,
    Ignore,
    UserHandler {
        handler: usize,  // 用户态函数地址
        mask: u32,
        flags: u32,
    },
}
```

- [ ] **Step 2: rt_sigaction**

```rust
/// sys_rt_sigaction — 注册/查询信号处理函数。
///
/// struct sigaction {
///     sa_handler: u64,
///     sa_flags: u64,
///     sa_restorer: u64,  // unused on riscv64
///     sa_mask: u64,
/// }
pub fn sys_rt_sigaction(sig: u32, act_addr: usize, oact_addr: usize, sigsetsize: usize) -> i64 {
    // 1. 如果 oact_addr != 0，将旧 action 写出
    // 2. 如果 act_addr != 0，读取新 action 并注册
}
```

- [ ] **Step 3: rt_sigprocmask**

```rust
/// sys_rt_sigprocmask — SIG_BLOCK / SIG_UNBLOCK / SIG_SETMASK
pub fn sys_rt_sigprocmask(how: u32, set_addr: usize, oset_addr: usize, sigsetsize: usize) -> i64 {
    // 操作 TCB 的 signal_mask
}
```

- [ ] **Step 4: rt_sigreturn（信号帧恢复）**

信号投递时在用户栈上构造 signal frame（保存 TrapContext），handler 返回后调用 `rt_sigreturn` 恢复。这是最复杂的部分——需要修改 trap return 路径检查是否有 pending signal 并构造 signal frame。

- [ ] **Step 5: Commit**

---

## Task 5: 当前工作目录 + 路径解析增强

- [ ] **Step 1: TCB 添加 cwd 字段**

```rust
/// 当前工作目录路径（默认 "/"）
cwd: sync::SpinLock<alloc::string::String>,
```

- [ ] **Step 2: sys_getcwd / sys_chdir**
- [ ] **Step 3: openat 使用 cwd 解析相对路径**
- [ ] **Step 4: Commit**

---

## Task 6: 更新 dispatch 表

- [ ] **Step 1: 更新 `src/syscall/abi.rs` 的 dispatch 函数**

```rust
pub fn dispatch(nr: u64, args: &SyscallArgs) -> i64 {
    match nr {
        // === 已有（P8） ===
        64  => sys_write(args...),
        93  => sys_exit(args...),

        // === 进程信息 ===
        94  => sys_exit_group(args.args[0] as i32),
        96  => sys_set_tid_address(args.args[0] as usize),
        172 => sys_getpid(),
        173 => sys_getppid(),
        174 | 175 => sys_getuid(),
        176 | 177 => sys_getgid(),

        // === 内存 ===
        214 => sys_brk(args.args[0] as usize),
        222 => sys_mmap(args...),
        215 => sys_munmap(args...),
        226 => sys_mprotect(args...),

        // === 文件 ===
        56  => sys_openat(args...),
        57  => sys_close(args...),
        63  => sys_read(args...),
        66  => sys_writev(args...),
        62  => sys_lseek(args...),
        79  => sys_fstatat(args...),
        80  => sys_fstat(args...),
        61  => sys_getdents64(args...),
        23  => sys_dup(args...),
        24  => sys_dup3(args...),
        59  => sys_pipe2(args...),
        25  => sys_fcntl(args...),
        29  => sys_ioctl(args...),
        17  => sys_getcwd(args...),
        49  => sys_chdir(args...),
        48  => sys_faccessat(args...),
        35  => sys_unlinkat(args...),
        34  => sys_mkdirat(args...),
        78  => sys_readlinkat(args...),

        // === 信号 ===
        134 => sys_rt_sigaction(args...),
        135 => sys_rt_sigprocmask(args...),
        139 => sys_rt_sigreturn(args...),

        // === 进程 ===
        220 => sys_clone(args...),
        221 => sys_execve(args...),
        260 => sys_wait4(args...),

        // === 系统 ===
        160 => sys_uname(args...),
        113 => sys_clock_gettime(args...),
        169 => sys_gettimeofday(args...),
        278 => sys_getrandom(args...),
        261 => 0, // prlimit64 stub

        _ => {
            log::warn!("未实现 syscall: nr={}", nr);
            -38 // ENOSYS
        }
    }
}
```

- [ ] **Step 2: 编译测试**
- [ ] **Step 3: Commit**

---

## 实现顺序建议

1. **Tier 1 先行**：info.rs → memory.rs（brk/mmap）→ signal.rs（sigaction/sigprocmask）→ uname
2. **Tier 2 文件**：openat → fstat → dup → pipe → getdents64 → writev → fcntl → ioctl
3. **Tier 3 补充**：在 BusyBox 调试过程中按 strace 输出逐个补充
