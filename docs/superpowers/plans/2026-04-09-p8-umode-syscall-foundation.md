# P8: Linux ABI 兼容层 — U-mode ecall 基础

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 U-mode 中运行一个最小静态二进制（ecall write + exit），验证完整的 trap → syscall → return 路径。

**Architecture:** 在 SAS 内核之上引入 U-mode 执行层。内核模块间仍使用直接函数调用（SAS 原有模型），用户程序通过 ecall/svc 触发 trap 进入内核 syscall dispatcher。共享同一页表（identity mapping），通过 PTE U-bit 区分内核/用户页。这是一个**混合模型**——内核内 SAS，内核-用户间 trap-based。

**Tech Stack:** Rust nightly (no_std), RISC-V64 + AArch64, 现有 TrapContext/trap_entry/trap_return 汇编基础设施（无需修改）。

**关键发现（已确认）：**
- 汇编层（`interrupt.S`）已完整支持 U-mode trap 路径（两个架构均完成）
- PTE flags 已有 `user_rw/user_rx/user_ro/user_rwx` preset（两个架构均完成）
- 只需修改 Rust 层：把 `HandleTrap` 中的 `panic!("ecall/SVC 不应触发")` 改为 dispatch

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Create | `src/syscall/abi.rs` | Raw ABI 层：u64 参数 → Rust 类型转换 + syscall dispatch |
| Modify | `src/syscall/mod.rs` | 新增 `pub mod abi` |
| Modify | `src/arch/riscv64/interrupt.rs:249-255` | ecall → dispatch（替换 panic） |
| Modify | `src/arch/aarch64/interrupt.rs:227-233` | SVC → dispatch（替换 panic） |
| Create | `src/user/mod.rs` | U-mode 进入机制 + 用户页映射 |
| Modify | `src/lib.rs` | 新增 `pub mod user` |
| Create | `tests/umode-test/Cargo.toml` | 独立 QEMU 测试包 |
| Create | `tests/umode-test/src/umode_hello.rs` | 测试：加载用户二进制 → U-mode → ecall → 验证 |
| Create | `tests/umode-test/user/hello_riscv64.S` | RISC-V 用户态 hello world（汇编） |
| Create | `tests/umode-test/user/hello_aarch64.S` | AArch64 用户态 hello world（汇编） |
| Create | `tests/umode-test/build.rs` | 交叉编译用户汇编 → 嵌入 flat binary |

---

## Task 1: Syscall ABI dispatch 层

Raw u64 参数 → 参数验证 → 转发到现有 syscall 实现。这是 trap handler 和现有 Rust syscall 之间的桥梁。

**Files:**
- Create: `src/syscall/abi.rs`
- Modify: `src/syscall/mod.rs`

- [ ] **Step 1: 创建 `src/syscall/abi.rs`**

```rust
//! 系统调用 ABI 层——trap handler 与 Rust syscall 实现之间的桥梁。
//!
//! 用户程序通过 ecall/svc 触发 trap，trap handler 提取寄存器参数后调用此模块。
//! 职责：syscall number 分发 + 原始 u64 参数到 Rust 类型的安全转换。

/// Syscall 参数（6 个通用寄存器）。
///
/// RISC-V: a0-a5 → args[0]-args[5]
/// AArch64: x0-x5 → args[0]-args[5]
pub struct SyscallArgs {
    pub args: [u64; 6],
}

/// 分发 syscall——根据 syscall number 转发到对应实现。
///
/// 返回值遵循 Linux 约定：≥0 成功，<0 错误码取反。
/// trap handler 将返回值写入 a0 (RISC-V) / x0 (AArch64)。
pub fn dispatch(nr: u64, args: &SyscallArgs) -> i64 {
    match nr {
        // write(fd, buf, len)
        64 => sys_write(
            args.args[0] as u32,
            args.args[1] as usize,
            args.args[2] as usize,
        ),
        // exit(code)
        93 => sys_exit(args.args[0] as i32),
        _ => {
            log::warn!("未实现的 syscall: nr={}, a0={:#x}", nr, args.args[0]);
            -38 // -ENOSYS
        }
    }
}

/// sys_write — 写文件描述符。
///
/// fd=1/2 直接输出到 arch console（stdout/stderr），
/// 其他 fd 通过 VFS 路径处理。
fn sys_write(fd: u32, buf_addr: usize, len: usize) -> i64 {
    match fd {
        // stdout / stderr → 直接输出到 arch console
        1 | 2 => {
            // SAFETY: 调用者（用户程序）保证 buf_addr 指向有效的用户态内存。
            // P8 阶段暂不做完整的用户地址验证（后续 Task 补充）。
            let buf = unsafe { core::slice::from_raw_parts(buf_addr as *const u8, len) };
            if let Ok(s) = core::str::from_utf8(buf) {
                // 使用 log 输出——复用现有 arch console 后端
                log::info!("{}", s.trim_end());
            } else {
                // 非 UTF-8 数据，逐字节输出
                for &b in buf {
                    log::info!("{}", b as char);
                }
            }
            len as i64
        }
        _ => {
            // 其他 fd 暂返回 EBADF
            -9 // -EBADF
        }
    }
}

/// sys_exit — 终止当前任务。
fn sys_exit(code: i32) -> i64 {
    crate::task::exit(code);
    // exit() 是 diverging function，不会返回
}
```

- [ ] **Step 2: 在 `src/syscall/mod.rs` 中注册模块**

在 `pub mod process;` 行之后添加：

```rust
pub mod abi;
```

- [ ] **Step 3: 验证编译**

Run: `cargo build --target riscv64gc-unknown-none-elf -p simplekernel 2>&1 | head -20`
Expected: 编译通过（`abi.rs` 中的函数暂未被调用，但类型检查通过）

- [ ] **Step 4: Commit**

```bash
git add src/syscall/abi.rs src/syscall/mod.rs
git commit --signoff -m "$(cat <<'EOF'
feat(syscall): 添加 ABI dispatch 层，桥接 trap handler 与 Rust syscall

用户程序通过 ecall/svc 触发 trap 后，trap handler 提取寄存器参数
调用 abi::dispatch()。当前支持 SYS_write(64) 和 SYS_exit(93)。
fd=1/2 直接输出到 arch console。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: RISC-V ecall trap 分发

把 `HandleTrap` 中 ecall 的 `panic!` 替换为 syscall dispatch。

**Files:**
- Modify: `src/arch/riscv64/interrupt.rs:249-255`

- [ ] **Step 1: 修改 ecall handler**

将 `src/arch/riscv64/interrupt.rs` 中的：

```rust
            // SAS 模式下 ecall 不应触发——所有 syscall 通过直接函数调用
            CAUSE_U_ECALL | CAUSE_S_ECALL => {
                panic!(
                    "ecall 在 SAS 模式下不应触发: sepc=0x{:x}, scause=0x{:x}",
                    ctx.sepc, ctx.scause
                );
            }
```

替换为：

```rust
            // U-mode ecall → syscall dispatch
            CAUSE_U_ECALL => {
                // sepc 指向 ecall 指令本身，返回时需跳过（ecall 为 4 字节）
                ctx.sepc += 4;

                let args = crate::syscall::abi::SyscallArgs {
                    args: [ctx.a0, ctx.a1, ctx.a2, ctx.a3, ctx.a4, ctx.a5],
                };
                let ret = crate::syscall::abi::dispatch(ctx.a7, &args);

                // 返回值写入 a0（Linux ABI 约定）
                ctx.a0 = ret as u64;
            }
            // S-mode ecall 仍然是编程错误——内核代码不应使用 ecall
            CAUSE_S_ECALL => {
                panic!(
                    "S-mode ecall 不应触发（内核代码应使用直接函数调用）: sepc=0x{:x}",
                    ctx.sepc
                );
            }
```

- [ ] **Step 2: 验证编译**

Run: `cargo build --target riscv64gc-unknown-none-elf -p simplekernel 2>&1 | head -20`
Expected: 编译通过

- [ ] **Step 3: Commit**

```bash
git add src/arch/riscv64/interrupt.rs
git commit --signoff -m "$(cat <<'EOF'
feat(arch/riscv64): ecall trap 分发到 syscall ABI 层

U-mode ecall 不再 panic，而是提取 a7(syscall nr) + a0-a5(args)
调用 abi::dispatch()，返回值写入 a0，sepc 跳过 ecall 指令。
S-mode ecall 保留 panic（内核代码不应使用 ecall）。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: AArch64 SVC trap 分发

把 `dispatch_sync` 中 SVC 的 `panic!` 替换为 syscall dispatch。

**Files:**
- Modify: `src/arch/aarch64/interrupt.rs:222-233`

- [ ] **Step 1: 修改 SVC handler**

将 `src/arch/aarch64/interrupt.rs` 中 `dispatch_sync` 函数的 EC=0x15 分支：

```rust
        // SAS 模式下 SVC 不应触发——所有 syscall 通过直接函数调用
        0x15 => {
            panic!(
                "SVC 在 SAS 模式下不应触发: ESR=0x{:x}, ELR=0x{:x}",
                esr, ctx.elr_el1
            );
        }
```

替换为：

```rust
        // EL0 SVC → syscall dispatch
        // AArch64: ELR_EL1 已指向 SVC 的下一条指令，无需手动 +4
        0x15 => {
            let args = crate::syscall::abi::SyscallArgs {
                args: [
                    ctx.x[0], ctx.x[1], ctx.x[2],
                    ctx.x[3], ctx.x[4], ctx.x[5],
                ],
            };
            let ret = crate::syscall::abi::dispatch(ctx.x[8], &args);

            // 返回值写入 x0（Linux ABI 约定）
            ctx.x[0] = ret as u64;
        }
```

- [ ] **Step 2: 确认 lower_el handler 路由到 dispatch_sync**

验证 `sync_lower_el_aarch64_handler` 调用 `dispatch_sync`。当前代码使用 `exception_handler!` 宏生成，确认 `sync_lower_el_aarch64_handler => sync` 在第 293 行。EL0 的 SVC 通过此路径到达 `dispatch_sync`。

- [ ] **Step 3: 验证编译**

Run: `cargo build --target aarch64-unknown-none-softfloat -p simplekernel 2>&1 | head -20`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src/arch/aarch64/interrupt.rs
git commit --signoff -m "$(cat <<'EOF'
feat(arch/aarch64): SVC trap 分发到 syscall ABI 层

EL0 SVC(EC=0x15) 不再 panic，而是提取 x8(syscall nr) + x0-x5(args)
调用 abi::dispatch()，返回值写入 x0。
AArch64 ELR_EL1 已指向 SVC 下一条指令，无需手动调整。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: U-mode 进入机制

提供从内核线程"跳入"U-mode 的机制：构造 TrapContext + 调用 trap_return。

**Files:**
- Create: `src/user/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: 创建 `src/user/mod.rs`**

```rust
//! 用户态执行支持——加载用户二进制、进入 U-mode。
//!
//! 采用混合模型：内核模块间 SAS（直接函数调用），
//! 用户程序通过 ecall/svc trap 进入内核。
//! 共享同一页表，通过 PTE U-bit 区分权限。

use memory_types::VirtAddr;

/// 进入用户态执行。
///
/// 在当前内核栈上构造 TrapContext，通过 trap_return（汇编）
/// 执行 sret/eret 下降到 U-mode。此函数不返回（对当前内核线程而言）。
///
/// # 参数
/// - `entry`: 用户程序入口地址（虚拟地址，identity mapping 下 == 物理地址）
/// - `user_sp`: 用户栈顶地址
///
/// # Safety
/// - `entry` 必须指向已映射且设置了 U-bit 的可执行页
/// - `user_sp` 必须指向已映射且设置了 U-bit 的可读写页的顶部
/// - 当前必须处于内核线程上下文中（非中断上下文）
pub unsafe fn enter_usermode(entry: usize, user_sp: usize) -> ! {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        enter_usermode_riscv64(entry, user_sp)
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        enter_usermode_aarch64(entry, user_sp)
    }
}

#[cfg(target_arch = "riscv64")]
unsafe fn enter_usermode_riscv64(entry: usize, user_sp: usize) -> ! {
    use crate::arch::riscv64::context::TrapContext;

    unsafe extern "C" {
        fn trap_return(ctx: *const TrapContext) -> !;
    }

    // 构造返回用户态的 sstatus：
    // - SPP = 0（返回 U-mode）
    // - SPIE = 1（sret 后中断使能）
    // SPP 是 bit 8，SPIE 是 bit 5
    let sstatus: u64 = 1 << 5; // SPIE=1, SPP=0

    let mut ctx = TrapContext::default();
    ctx.sepc = entry as u64;
    ctx.sp = user_sp as u64;
    ctx.sstatus = sstatus;

    // 设置 sscratch = 当前内核栈指针。
    // trap_entry 通过 csrrw sp, sscratch, sp 交换 sp 和 sscratch，
    // 用户态 trap 时 sscratch 提供内核栈地址。
    let kernel_sp: usize;
    core::arch::asm!("mv {}, sp", out(reg) kernel_sp);
    core::arch::asm!("csrw sscratch, {}", in(reg) kernel_sp);

    // RISC-V: 设置 sstatus.SUM = 1，允许 S-mode 访问 U-mode 页。
    // 用于 sys_write 等需要读取用户内存的 syscall。
    // SUM 是 sstatus bit 18
    core::arch::asm!("csrs sstatus, {}", in(reg) 1usize << 18);

    // 将 TrapContext 放到栈上，传给 trap_return
    // trap_return 期望 a0/x0 = TrapContext 指针
    unsafe { trap_return(&ctx as *const TrapContext) }
}

#[cfg(target_arch = "aarch64")]
unsafe fn enter_usermode_aarch64(entry: usize, user_sp: usize) -> ! {
    use crate::arch::aarch64::context::TrapContext;

    unsafe extern "C" {
        fn trap_return(ctx: *const TrapContext) -> !;
    }

    // 构造返回 EL0 的 SPSR_EL1：
    // - M[3:0] = 0b0000（EL0t）
    // - DAIF = 0（EL0 下中断使能）
    let spsr_el1: u64 = 0; // EL0t, all interrupts enabled

    let mut ctx = TrapContext::default();
    ctx.elr_el1 = entry as u64;
    ctx.sp_el0 = user_sp as u64;
    ctx.spsr_el1 = spsr_el1;

    // 将 TrapContext 放到栈上，传给 trap_return
    unsafe { trap_return(&ctx as *const TrapContext) }
}

/// 在内核页表中将指定物理地址范围映射为用户可访问页。
///
/// 使用 identity mapping（VA == PA），设置 U-bit 使 U-mode 可访问。
///
/// # 参数
/// - `paddr`: 起始物理地址（页对齐）
/// - `page_count`: 页数
/// - `executable`: 是否需要执行权限
pub fn map_user_pages(paddr: memory_types::PhysAddr, page_count: usize, executable: bool) {
    use paging::{PteFlags, PteFlagsOps};

    let flags = if executable {
        PteFlags::user_rx()
    } else {
        PteFlags::user_rw()
    };

    let mut pt = paging::kernel_page_table().lock();
    let start = paddr;
    let end = paddr + page_count * config::PAGE_SIZE;
    pt.identity_map_range(start, end, flags);
    log::info!(
        "UserMap: {}-{} ({} pages, {})",
        start,
        end,
        page_count,
        if executable { "RX" } else { "RW" }
    );
}
```

- [ ] **Step 2: 在 `src/lib.rs` 中注册模块**

在 `pub mod task;` 行之后添加：

```rust
pub mod user;
```

- [ ] **Step 3: 修正 `trap_return` 的签名声明**

验证 `trap_return` 在汇编中是 global 符号，两个架构的 `interrupt.S` 均有 `.global trap_return`。确认上述 `extern "C"` 声明匹配。

`trap_return` 接收一个参数：
- RISC-V: `a0 = TrapContext*`（`mv sp, a0`）
- AArch64: `x0 = TrapContext*`（`mov sp, x0`）

`trap_return` 为 diverging（通过 sret/eret 离开，不返回到调用者），声明为 `-> !`。

- [ ] **Step 4: 验证编译（两个架构）**

Run: `cargo build --target riscv64gc-unknown-none-elf -p simplekernel 2>&1 | head -20`
Run: `cargo build --target aarch64-unknown-none-softfloat -p simplekernel 2>&1 | head -20`
Expected: 均通过

- [ ] **Step 5: Commit**

```bash
git add src/user/mod.rs src/lib.rs
git commit --signoff -m "$(cat <<'EOF'
feat(user): 添加 U-mode 进入机制

提供 enter_usermode(entry, user_sp) 和 map_user_pages()。
在内核栈上构造 TrapContext（sstatus.SPP=0 / spsr_el1.M=EL0），
通过汇编 trap_return 执行 sret/eret 下降到 U-mode。
设置 sstatus.SUM（RISC-V）允许内核访问用户页。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: 用户态 Hello World 测试二进制

创建一个最小的用户态汇编程序，通过 ecall/svc 调用 write + exit。然后创建独立 QEMU 测试来加载并运行它。

**Files:**
- Create: `tests/umode-test/Cargo.toml`
- Create: `tests/umode-test/src/umode_hello.rs`
- Create: `tests/umode-test/user/hello_riscv64.S`
- Create: `tests/umode-test/user/hello_aarch64.S`
- Create: `tests/umode-test/build.rs`
- Modify: `Cargo.toml`（workspace members）

- [ ] **Step 1: 创建用户态汇编（RISC-V）**

`tests/umode-test/user/hello_riscv64.S`:

```asm
# 最小用户态程序——通过 ecall 调用 write + exit
#
# Linux ABI:
#   a7 = syscall number
#   a0-a5 = arguments
#   ecall 触发 trap

.section .text
.global _start
_start:
    # write(1, msg, 20)
    li a0, 1                # fd = stdout
    # PC-relative 获取 msg 地址
    la a1, msg              # buf
    li a2, 20               # len = 20
    li a7, 64               # __NR_write
    ecall

    # exit(0)
    li a0, 0                # code = 0
    li a7, 93               # __NR_exit
    ecall

    # 不应到达此处
    j .

.section .rodata
msg:
    .ascii "Hello from U-mode!\n\0"
```

- [ ] **Step 2: 创建用户态汇编（AArch64）**

`tests/umode-test/user/hello_aarch64.S`:

```asm
// 最小用户态程序——通过 svc 调用 write + exit
//
// Linux ABI:
//   x8 = syscall number
//   x0-x5 = arguments
//   svc #0 触发 trap

.section .text
.global _start
_start:
    // write(1, msg, 20)
    mov x0, #1              // fd = stdout
    adrp x1, msg            // buf (PC-relative)
    add x1, x1, :lo12:msg
    mov x2, #20             // len = 20
    mov x8, #64             // __NR_write
    svc #0

    // exit(0)
    mov x0, #0              // code = 0
    mov x8, #93             // __NR_exit
    svc #0

    // 不应到达此处
    b .

.section .rodata
msg:
    .ascii "Hello from U-mode!\n\0"
```

- [ ] **Step 3: 创建 `build.rs`**

`tests/umode-test/build.rs` — 交叉编译用户汇编为 flat binary：

```rust
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let user_dir = manifest_dir.join("user");

    let target = env::var("TARGET").unwrap_or_default();

    let (src, prefix, link_addr) = if target.contains("riscv64") {
        ("hello_riscv64.S", "riscv64-linux-gnu-", "0x80400000")
    } else if target.contains("aarch64") {
        ("hello_aarch64.S", "aarch64-linux-gnu-", "0x40200000")
    } else {
        // Host target (cargo test), skip
        return;
    };

    let src_path = user_dir.join(src);
    let obj_path = out_dir.join("hello_user.o");
    let bin_path = out_dir.join("hello_user.bin");

    // 汇编
    let as_cmd = format!("{}as", prefix);
    let status = Command::new(&as_cmd)
        .args([
            src_path.to_str().expect("source path"),
            "-o",
            obj_path.to_str().expect("object path"),
        ])
        .status()
        .unwrap_or_else(|e| panic!("运行 {} 失败: {}", as_cmd, e));
    assert!(status.success(), "{} 汇编失败", as_cmd);

    // 链接为 flat binary
    let ld_cmd = format!("{}ld", prefix);
    let status = Command::new(&ld_cmd)
        .args([
            &format!("-Ttext={}", link_addr),
            "--oformat",
            "binary",
            "-o",
            bin_path.to_str().expect("binary path"),
            obj_path.to_str().expect("object path"),
        ])
        .status()
        .unwrap_or_else(|e| panic!("运行 {} 失败: {}", ld_cmd, e));
    assert!(status.success(), "{} 链接失败", ld_cmd);

    // 输出给 include_bytes! 用
    println!("cargo:rustc-env=USER_HELLO_BIN={}", bin_path.display());
    println!("cargo:rerun-if-changed={}", src_path.display());
}
```

- [ ] **Step 4: 创建 `Cargo.toml`**

`tests/umode-test/Cargo.toml`:

```toml
[package]
name = "umode-test"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "umode-hello"
path = "src/umode_hello.rs"
test = false

[dependencies]
simplekernel = { path = "../.." }
test_harness = { path = "../test_harness" }
log = "0.4"
```

- [ ] **Step 5: 创建测试入口**

`tests/umode-test/src/umode_hello.rs`:

```rust
#![no_std]
#![no_main]
#![feature(sync_unsafe_cell)]

extern crate alloc;

use simplekernel::task;
use simplekernel::user;

/// 用户二进制（由 build.rs 交叉编译嵌入）
const USER_HELLO: &[u8] = include_bytes!(env!("USER_HELLO_BIN"));

/// 用户二进制的链接地址（与 build.rs 中的 link_addr 一致）
#[cfg(target_arch = "riscv64")]
const USER_LOAD_ADDR: usize = 0x80400000;
#[cfg(target_arch = "aarch64")]
const USER_LOAD_ADDR: usize = 0x40200000;

/// 用户栈大小（16KB）
const USER_STACK_SIZE: usize = 16 * 1024;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test);

/// 测试主函数——在内核线程中加载并运行用户二进制。
fn run_test() {
    log::info!("umode-hello: spawning user task...");

    task::spawn_kernel_thread("user_runner", user_runner_thread, 0);

    // 让出 CPU 让 user_runner_thread 运行
    for _ in 0..100 {
        simplekernel::syscall::process::yield_now();
    }

    log::info!("=== UMODE HELLO TEST PASSED ===");
}

/// 内核线程：加载用户二进制并进入 U-mode。
fn user_runner_thread(_arg: usize) {
    log::info!(
        "user_runner: loading {} bytes to {:#x}",
        USER_HELLO.len(),
        USER_LOAD_ADDR
    );

    // 1. 将用户二进制复制到目标地址
    let dst = USER_LOAD_ADDR as *mut u8;
    // SAFETY: USER_LOAD_ADDR 在内核已映射的 RAM 范围内（RW），
    // 且与内核代码段不重叠（位于空闲内存区域）
    unsafe {
        core::ptr::copy_nonoverlapping(USER_HELLO.as_ptr(), dst, USER_HELLO.len());
    }

    // 2. 映射用户代码页（RX）
    let code_pages = (USER_HELLO.len() + config::PAGE_SIZE - 1) / config::PAGE_SIZE;
    user::map_user_pages(
        memory_types::PhysAddr::new(USER_LOAD_ADDR),
        code_pages,
        true, // executable
    );

    // 3. 分配并映射用户栈（RW）
    let stack_pages = USER_STACK_SIZE / config::PAGE_SIZE;
    let stack_frames =
        memory::frame::AllocatedFrames::<memory_types::Page4K>::alloc(stack_pages)
            .expect("用户栈分配失败");
    let stack_base = stack_frames.start_paddr();
    user::map_user_pages(stack_base, stack_pages, false);
    let user_sp = stack_base.as_usize() + USER_STACK_SIZE;

    log::info!(
        "user_runner: code={:#x}({}p), stack={:#x}-{:#x}",
        USER_LOAD_ADDR,
        code_pages,
        stack_base.as_usize(),
        user_sp
    );

    // 4. 进入 U-mode（此函数不返回）
    // SAFETY: 代码和栈页已正确映射，entry 指向有效用户代码
    unsafe {
        user::enter_usermode(USER_LOAD_ADDR, user_sp);
    }
}
```

- [ ] **Step 6: 将测试包加入 workspace**

在根 `Cargo.toml` 的 `[workspace] members` 中添加 `"tests/umode-test"`。

- [ ] **Step 7: 验证编译**

Run: `cargo build --target riscv64gc-unknown-none-elf -p umode-test 2>&1 | tail -10`
Expected: 编译通过（需要安装 `riscv64-linux-gnu-binutils`）

如果 cross-toolchain 不可用，用 `cargo build --target riscv64gc-unknown-none-elf -p simplekernel` 先验证内核部分编译通过。

- [ ] **Step 8: 运行 QEMU 测试**

Run: `cargo xtask test --arch riscv64 --name umode-hello 2>&1 | tail -30`
Expected output 包含: `Hello from U-mode!` 和 `UMODE HELLO TEST PASSED`

- [ ] **Step 9: Commit**

```bash
git add tests/umode-test/ Cargo.toml
git commit --signoff -m "$(cat <<'EOF'
feat(tests): 添加 U-mode hello world 独立 QEMU 测试

验证完整的 U-mode 执行路径：
  内核线程 → 加载用户二进制 → map U-bit 页
  → enter_usermode(sret/eret) → U-mode ecall write
  → trap dispatch → console 输出 → ecall exit

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: 帧分配器泄漏防护

`enter_usermode` 是 diverging function（不返回），Task 5 中分配的 `stack_frames` 会被 drop 释放帧——但我们仍需要这些帧。需要 `mem::forget` 或持有所有权。

**Files:**
- Modify: `tests/umode-test/src/umode_hello.rs`

- [ ] **Step 1: 防止栈帧被回收**

在 `user_runner_thread` 中，分配用户栈帧后添加 `mem::forget`：

```rust
    let stack_frames =
        memory::frame::AllocatedFrames::<memory_types::Page4K>::alloc(stack_pages)
            .expect("用户栈分配失败");
    let stack_base = stack_frames.start_paddr();
    user::map_user_pages(stack_base, stack_pages, false);
    let user_sp = stack_base.as_usize() + USER_STACK_SIZE;

    // 阻止帧被 drop 回收——用户栈在进程退出前必须保持有效
    core::mem::forget(stack_frames);
```

- [ ] **Step 2: 验证编译并运行**

Run: `cargo xtask test --arch riscv64 --name umode-hello`

- [ ] **Step 3: Commit**

```bash
git add tests/umode-test/src/umode_hello.rs
git commit --signoff -m "$(cat <<'EOF'
fix(tests/umode): mem::forget 用户栈帧，防止 drop 回收

enter_usermode 是 diverging function，局部变量的析构器不会运行，
但编译器仍可能在函数尾部插入 drop。显式 forget 确保帧不被释放。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Trouble-Shooting Checklist

实施中可能遇到的问题及解决方案：

| 症状 | 原因 | 解决 |
|------|------|------|
| `ecall` 后直接跳回内核 panic | `sstatus.SPP` 未清零，`sret` 返回 S-mode 而非 U-mode | 检查 `enter_usermode_riscv64` 中 `sstatus` 是否正确设置 SPP=0 |
| Page fault on user code address | 用户页未映射或缺少 U-bit | 检查 `map_user_pages` 的 flags 是否使用 `user_rx()` |
| 用户程序 `la` 指令加载到错误地址 | 二进制 link 地址与实际加载地址不匹配 | 确认 build.rs 的 `-Ttext` 与 `USER_LOAD_ADDR` 一致 |
| `HandleTrap: 异常 code=12/13/15` (page fault) | S-mode 访问 U-mode 页但 SUM=0 | 确认 `enter_usermode` 中设置了 `sstatus.SUM=1` |
| `sscratch` 未正确设置，内核态 trap 损坏 sp | `enter_usermode` 未写 sscratch | 确认 `csrw sscratch, kernel_sp` 在 sret 前执行 |
| AArch64 EL0 执行 page fault | TTBR0_EL1 未指向包含用户映射的页表 | `enter_usermode_aarch64` 中设置 `ctx.ttbr0_el1` |
| Cross-toolchain 不可用 | 未安装 riscv64-linux-gnu-binutils | `apt install binutils-riscv64-linux-gnu` |

---

## Phase Overview（后续阶段概要）

P8 完成后，到 BusyBox 的路线图：

### P9: ELF 加载器 + execve（预计 3000-4000 行）
- 解析 ELF64 PT_LOAD 段，映射到用户地址空间
- Per-process 页表（脱离 identity mapping 限制）
- `execve` syscall：加载 ELF → 替换地址空间 → 进入 U-mode
- 用户栈初始化：argc, argv, envp, auxv（musl 启动依赖）
- `fork`/`clone`：复制页表 + COW（可选）

### P10: 核心 syscall 扩充（预计 4000-5000 行）
- 内存：`brk`, `mmap`, `munmap`, `mprotect`
- 文件：`openat`, `fstat`, `dup2`, `pipe2`, `fcntl`, `ioctl`, `getcwd`, `chdir`, `getdents64`, `readlinkat`
- 进程：`getpid`, `getppid`, `wait4`, `set_tid_address`, `getuid`/`getgid`（stub）
- 信号：`rt_sigaction`, `rt_sigprocmask`, `rt_sigreturn`
- 系统：`uname`, `clock_gettime`

### P11: 设备文件 + 伪文件系统（预计 2000-3000 行）
- CharDevice trait + /dev/null, /dev/zero, /dev/console
- devtmpfs 挂载
- TTY 基础（ioctl TCGETS/TCSETS/TIOCGWINSZ）
- procfs 最小集（/proc/self/exe, /proc/mounts）
- stdin/stdout/stderr → /dev/console

### P12: BusyBox 集成（预计 1000-2000 行 + 大量调试）
- 交叉编译 BusyBox (musl static, riscv64)
- 创建 initramfs 镜像
- 内核加载 /init → BusyBox sh
- strace 式调试：逐个补充缺失 syscall
