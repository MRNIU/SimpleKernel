# 测试隔离迁移：每测试独立 QEMU 实例 + 内核冒烟测试

> **已完成 / 已过时**：此计划已实施完毕。后续重构将 `tests/standalone/` 扁平化为 `tests/`，
> `crates/test_harness/` 移至 `tests/test_harness/`，各测试 `build.rs` 已删除（链接参数
> 移至 `.cargo/config.toml`）。以下内容中的路径和步骤不再反映当前代码结构。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 统一内核启动路径（消除 `main.rs` 与 `boot.rs` 的重复），将内核 crate 的 49 个宿主机测试迁移为独立 QEMU 二进制（每个测试一个干净的内核实例），删除所有 host mock / test-support 配置代码，并在内核启动流程中加入冒烟测试。

**Architecture:** 首先统一 `main.rs::bootstrap()` 和 `boot.rs::kernel_init()` 为单一启动路径——`main.rs` 调用 `kernel_init(Full)` 而非重复初始化逻辑。ELF parser init 合入 `kernel_init()`。每个测试是 `tests/standalone/` 下的独立 crate，通过 `test_harness` crate 提供入口宏消除样板代码。`xtask test` 并行启动多个 QEMU 实例执行测试。`should_panic` 测试复用内核 panic handler（打印栈回溯），`xtask` 侧通过检查串口输出中的 panic 消息判断成功/失败。

**Tech Stack:** Rust nightly (`no_std`/`no_main`), QEMU, xshell (xtask), `proc-macro` 或声明宏 (test_harness)

**参考决策:** ADR-004（内核代码只为裸机编译）

---

## 文件结构总览

### 新建文件

| 路径 | 职责 |
|------|------|
| `crates/test_harness/Cargo.toml` | 测试 harness crate 包定义 |
| `crates/test_harness/src/lib.rs` | `test_main!` 宏 + `exit_qemu()` + panic handler |
| `crates/test_harness/build.rs` | 汇编编译 + 链接器脚本（复用 system-test 的 build.rs） |
| `tests/standalone/<test_name>/Cargo.toml` | 每个测试的 crate（共 ~20 个，相关测试合并为一个二进制） |
| `tests/standalone/<test_name>/src/main.rs` | 每个测试的入口 |

### 修改文件

| 路径 | 变更 |
|------|------|
| `src/boot.rs` | 统一为单一启动入口；合入 ELF parser init；加入冒烟断言 |
| `src/main.rs` | `bootstrap()` 简化为调用 `kernel_init(Full)` + spawn smoke threads + idle loop |
| `src/smoke_test.rs` | 删除 phase2/phase3/phase4（逻辑合入 boot.rs），只保留 `spawn_all()` |
| `xtask/src/test.rs` | 并行 QEMU + should_panic 输出检测 + 新的测试发现 |
| `xtask/src/main.rs` | `--jobs` 参数 + `--filter` 参数 |
| `.github/workflows/workflow.yml` | `cargo test` 缩小范围到纯逻辑 crate |
| `Cargo.toml`（workspace） | 添加 test_harness 和新测试 crate 为 workspace member |

### 删除文件/代码

| 路径 | 删除内容 |
|------|---------|
| `crates/frame_allocator/src/lib.rs` | `ensure_test_init()`、`cfg_attr(not(test), no_std)` → `#![no_std]` |
| `crates/frame_allocator/src/transitions.rs` | `#[cfg(test)] mod tests` 整个模块 |
| `crates/frame_allocator/Cargo.toml` | `test-support` feature |
| `crates/paging/src/lib.rs` | `ensure_test_init()`、`#[cfg(test)] mod tests` |
| `crates/paging/src/table.rs` | `#[cfg(test)] mod tests`（29 个测试） |
| `crates/paging/src/mapping.rs` | `#[cfg(test)] mod tests`（5 个测试） |
| `crates/paging/Cargo.toml` | `test-support` feature、`[dev-dependencies]` |
| `crates/sync/src/lib.rs` | `cfg_attr(not(test), no_std)` → `#![no_std]` |
| `crates/sync/src/raw/ttas.rs` | `#[cfg(test)]` / `#[cfg(not(test))]` 双路径、`#[cfg(test)] mod tests` |
| `crates/sync/src/lock_stack.rs` | `#[cfg(test)] mod tests` |
| `crates/memory/src/vma.rs` | `#[cfg(test)] mod tests` |
| `crates/memory/Cargo.toml` | `[dev-dependencies]` |
| `crates/per_cpu/src/host.rs` | 整个文件 |
| `crates/per_cpu/src/lib.rs` | `cfg(not(bare_metal)) mod host` 分支 |
| `crates/local_tick/src/lib.rs` | `cfg_attr(not(test), no_std)` → `#![no_std]` |
| `src/panic.rs` | `#[cfg(test)]` 双路径（handle_panic mock、raw_dump_stack mock、tests 模块） |
| `tests/system/` | 整个目录（统一测试内核被独立二进制取代） |

---

## Task 0: 统一内核启动路径

**Files:**
- Modify: `src/boot.rs`
- Modify: `src/main.rs`
- Modify: `src/smoke_test.rs`

### 设计说明

当前 `main.rs::bootstrap()` 和 `boot.rs::kernel_init()` 是同一段初始化逻辑的两份拷贝，差异仅在：
1. `bootstrap()` 在 `early_init()` 后调用 `smoke_test::phase2()`（包含 ELF parser init + SpinLock 冒烟）
2. `bootstrap()` 在 memory init 后调用 `smoke_test::phase3()`（堆分配冒烟 + aarch64 TLBI 测试）
3. `bootstrap()` 在 wake_secondary_cores 后调用 `smoke_test::phase4()` + `spawn_all()`

统一方案：
- **ELF parser init** 合入 `kernel_init()` 的 Memory 阶段（在 `early_init()` 之后，memory::init() 之前——此时已有 `MEMORY_INFO`）
- **冒烟断言** 合入 `kernel_init()` 各阶段末尾（始终运行，开销可忽略）
- **`bootstrap()` 简化为** `kernel_init(Full)` → `smoke_test::spawn_all()` → `schedule()` → idle loop
- **`smoke_test.rs` 删除 phase2/phase3/phase4**，只保留 `spawn_all()`（线程级冒烟测试）

- [ ] **Step 1: 修改 `src/boot.rs`——合入 ELF parser init 和冒烟断言**

```rust
pub unsafe fn kernel_init(argc: i32, argv: *const *const u8, level: InitLevel) {
    crate::logging::init();
    unsafe { per_cpu::percpu_init() };
    crate::init::early_init(Arch::dtb_addr(argc, argv));

    // ELF parser——用于 panic 栈回溯符号解析
    let elf_addr = memory::MEMORY_INFO
        .get()
        .expect("MEMORY_INFO not initialized")
        .kernel_addr
        .as_usize() as u64;
    // SAFETY: elf_addr 是内核自身的 ELF 基地址，在内核生命周期内有效
    unsafe { crate::panic::init_elf(elf_addr) };

    // 冒烟测试：SpinLock 基本功能（此时中断已禁用，锁操作安全）
    {
        let lock = sync::SpinLock::new(42u32, "smoke_test", sync::lock_level::UNSPECIFIED);
        let guard = lock.lock();
        assert_eq!(*guard, 42, "冒烟测试: SpinLock 基本功能失败");
        drop(guard);
        assert!(!lock.is_locked(), "冒烟测试: SpinLock 释放后仍被持有");
        log::debug!("冒烟测试: SpinLock OK");
    }

    let mut kernel_as = memory::init();
    Arch::map_early_mmio(&mut kernel_as).expect("failed to map early MMIO");
    {
        let pt = paging::kernel_page_table().lock();
        unsafe { Arch::activate_page_table(&pt) };
    }
    log::info!("MemoryInit: paging enabled");
    memory::store_kernel_address_space(kernel_as);

    // 冒烟测试：堆分配 + 帧分配
    {
        let v = alloc::vec![1u32, 2, 3];
        assert_eq!(v.len(), 3, "冒烟测试: 堆分配失败");
        let frame = frame_allocator::AllocatedFrames::alloc_one()
            .expect("冒烟测试: 帧分配失败");
        assert!(frame.start_paddr().is_aligned(), "冒烟测试: 帧未对齐");
        log::debug!("冒烟测试: 内存子系统 OK");
    }

    // aarch64 TLBI 封装验证
    #[cfg(target_arch = "aarch64")]
    {
        use aarch64_cpu::asm::{barrier, tlbi};
        barrier::dsb(barrier::SY);
        tlbi::vmalle1();
        barrier::dsb(barrier::SY);
        barrier::isb(barrier::SY);
        log::debug!("冒烟测试: TLBI OK");
    }

    if matches!(level, InitLevel::Memory) {
        return;
    }

    Arch::init_timer();
    Arch::init_interrupt();

    if matches!(level, InitLevel::Interrupt) {
        return;
    }

    crate::task::init();
    crate::device::device_init();
    crate::fs::fs_init();
    Arch::wake_secondary_cores();
}
```

注意：TLBI 的详细测试（vae1、vale1、aside1 等）从冒烟测试中移除——它们应该作为独立的 QEMU 测试。冒烟测试只保留最基本的 `vmalle1()`。

- [ ] **Step 2: 简化 `src/main.rs::bootstrap()`**

```rust
fn bootstrap(argc: i32, argv: *const *const u8) -> ! {
    // SAFETY: bare-metal 环境，主核首次调用
    unsafe {
        boot::kernel_init(argc, argv, boot::InitLevel::Full);
    }

    // 冒烟测试——启动线程级测试（锁竞争、sleep、clone/wait、signal、VFS）
    smoke_test::spawn_all();

    // 立即尝试调度，开始运行刚创建的线程
    task::schedule();

    // Idle loop — bootstrap 上下文成为 idle 任务
    loop {
        if preempt::check_and_clear_need_resched() {
            task::schedule();
        }
        core::hint::spin_loop();
    }
}
```

- [ ] **Step 3: 精简 `src/smoke_test.rs`——删除 phase2/phase3/phase4**

删除 `phase2()`、`phase3()`、`phase4()` 函数。保留 `spawn_all()` 及其关联的线程函数（`counter_thread`、`verifier_thread`、`p5b_test_thread`、`p6p7_test_thread` 等）。

- [ ] **Step 4: 验证裸机构建和运行**

```bash
cargo xtask build --arch riscv64
cargo xtask run --arch riscv64    # 确认启动流程、冒烟测试、线程测试正常
cargo xtask build --arch aarch64
cargo xtask run --arch aarch64    # 确认 TLBI 冒烟测试正常
```

- [ ] **Step 5: Commit**

```bash
git add src/boot.rs src/main.rs src/smoke_test.rs
git commit --signoff -m "refactor(boot): 统一启动路径，消除 main.rs 与 boot.rs 的初始化重复"
```

---

## Task 1: 创建 test_harness crate

**Files:**
- Create: `crates/test_harness/Cargo.toml`
- Create: `crates/test_harness/src/lib.rs`
- Create: `crates/test_harness/build.rs`
- Modify: `Cargo.toml`（workspace members）

### 设计说明

`test_harness` 提供一个 `test_main!` 宏，生成：
- `_start` 入口（区分主核/从核）
- 调用 `kernel_init(level)` 初始化内核
- 调用用户定义的测试函数
- 测试函数正常返回 → `exit_qemu(0)`（成功）
- 测试函数 panic → 内核 panic handler 打印栈回溯 → QEMU 挂起 → xtask 超时判定失败

对于 `should_panic` 测试，测试函数中触发 panic → panic handler 打印消息 → xtask 在输出中检测到预期消息 → 判定成功。`test_harness` 中通过设置一个 panic hook 在打印完消息后调用 `exit_qemu(0)`。

```
普通测试:   _start → kernel_init → test_fn() → exit_qemu(0)
                                      ↓ panic
                                   handle_panic → 打印栈回溯 → 死循环 → xtask 超时 = FAIL

should_panic: _start → kernel_init → test_fn() → panic!("expected msg")
                                                    ↓
                                   panic hook → 打印 "PANIC" + msg → exit_qemu(0) = PASS
              如果 test_fn 正常返回 → exit_qemu(1) = FAIL
```

- [ ] **Step 1: 创建 `crates/test_harness/Cargo.toml`**

```toml
[package]
name = "test_harness"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Test harness for standalone QEMU test binaries"

[dependencies]
simplekernel = { path = "../../", default-features = false }
per_cpu = { path = "../per_cpu" }
heap = { path = "../heap" }
memory = { path = "../memory" }
config = { path = "../config" }
log.workspace = true

[target.'cfg(target_arch = "riscv64")'.dependencies]
sbi-rt.workspace = true

[target.'cfg(target_arch = "aarch64")'.dependencies]
aarch64-cpu.workspace = true

[lints]
workspace = true
```

- [ ] **Step 2: 创建 `crates/test_harness/build.rs`**

直接复用 `tests/system/build.rs` 的内容（编译汇编 + 链接器脚本）。复制 `tests/system/build.rs` 到 `crates/test_harness/build.rs`，将 `../../src/arch` 路径改为适配 crate 位置。

注意：build.rs 中的路径是相对于 `CARGO_MANIFEST_DIR` 的。`crates/test_harness/` 到 `src/arch/` 的路径是 `../../src/arch`。与 `tests/system/` 到 `src/arch/` 的相对路径相同，因此可直接复制。

- [ ] **Step 3: 创建 `crates/test_harness/src/lib.rs`**

```rust
//! 独立 QEMU 测试二进制的公共 harness。
//!
//! 提供 `test_main!` 宏，消除每个测试二进制的样板代码
//! （`_start`、`panic_handler`、`exit_qemu`、`kernel_thread_bootstrap`）。

#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::alloc::Layout;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

/// 退出 QEMU，`code` 为退出码（0 = 成功，非零 = 失败）
pub fn exit_qemu(code: u32) -> ! {
    #[cfg(target_arch = "riscv64")]
    {
        let _ = code;
        sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
        loop {
            core::hint::spin_loop();
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        let _ = code;
        // SAFETY: PSCI SYSTEM_OFF 是标准固件接口
        unsafe {
            core::arch::asm!(
                "smc #0",
                in("x0") 0x8400_0008u64,
                in("x1") 0u64,
                in("x2") 0u64,
                in("x3") 0u64,
                options(nomem, nostack),
            );
        }
        loop {
            core::hint::spin_loop();
        }
    }

    #[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
    {
        let _ = code;
        loop {
            core::hint::spin_loop();
        }
    }
}

/// 生成测试二进制的入口和样板代码。
///
/// # 普通测试
/// ```ignore
/// test_harness::test_main!(simplekernel::boot::InitLevel::Full, test_fn);
///
/// fn test_fn() {
///     assert_eq!(1 + 1, 2);
/// }
/// ```
///
/// # should_panic 测试
/// ```ignore
/// test_harness::test_main!(simplekernel::boot::InitLevel::Memory, test_fn, should_panic);
///
/// fn test_fn() {
///     panic!("expected panic message");
/// }
/// ```
#[macro_export]
macro_rules! test_main {
    // 普通测试：test_fn 正常返回 → 成功，panic → 失败
    ($level:expr, $test_fn:ident) => {
        static PRIMARY_BOOTED: core::sync::atomic::AtomicBool =
            core::sync::atomic::AtomicBool::new(false);

        #[unsafe(no_mangle)]
        pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
            if !PRIMARY_BOOTED.swap(true, core::sync::atomic::Ordering::AcqRel) {
                // SAFETY: bare-metal 环境，主核首次调用
                unsafe {
                    simplekernel::boot::kernel_init(argc, argv, $level);
                }
                $test_fn();
                $crate::exit_qemu(0);
            } else {
                // 从核：初始化后空转
                unsafe { simplekernel::boot::kernel_init_smp() };
                loop {
                    core::hint::spin_loop();
                }
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn kernel_thread_bootstrap(_entry: usize, _arg: usize) -> ! {
            loop {
                core::hint::spin_loop();
            }
        }

        #[panic_handler]
        fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
            simplekernel::panic::handle_panic(info);
        }

        #[alloc_error_handler]
        fn alloc_error(layout: core::alloc::Layout) -> ! {
            simplekernel::logging::raw_put("TEST PANIC: alloc error\n");
            let _ = layout;
            $crate::exit_qemu(1);
        }
    };

    // should_panic 测试：test_fn panic → 成功（exit 0），正常返回 → 失败（exit 1）
    ($level:expr, $test_fn:ident, should_panic) => {
        static PRIMARY_BOOTED: core::sync::atomic::AtomicBool =
            core::sync::atomic::AtomicBool::new(false);

        #[unsafe(no_mangle)]
        pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
            if !PRIMARY_BOOTED.swap(true, core::sync::atomic::Ordering::AcqRel) {
                unsafe {
                    simplekernel::boot::kernel_init(argc, argv, $level);
                }
                $test_fn();
                // 如果执行到这里，说明没有 panic → 测试失败
                simplekernel::logging::raw_put("SHOULD_PANIC test returned without panic\n");
                $crate::exit_qemu(1);
            } else {
                unsafe { simplekernel::boot::kernel_init_smp() };
                loop {
                    core::hint::spin_loop();
                }
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn kernel_thread_bootstrap(_entry: usize, _arg: usize) -> ! {
            loop {
                core::hint::spin_loop();
            }
        }

        #[panic_handler]
        fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
            // 打印 panic 信息（xtask 通过输出匹配预期消息）
            simplekernel::logging::raw_put("PANIC at ");
            if let Some(loc) = info.location() {
                let mut buf = heapless::String::<256>::new();
                let _ = core::fmt::Write::write_fmt(&mut buf, format_args!(
                    "{}:{}", loc.file(), loc.line()
                ));
                simplekernel::logging::raw_put(buf.as_str());
            }
            simplekernel::logging::raw_put(": ");
            let mut msg_buf = heapless::String::<256>::new();
            let _ = core::fmt::Write::write_fmt(&mut msg_buf, format_args!(
                "{}", info.message()
            ));
            simplekernel::logging::raw_put(msg_buf.as_str());
            simplekernel::logging::raw_put("\n");
            // panic 发生 = should_panic 测试成功
            $crate::exit_qemu(0);
        }

        #[alloc_error_handler]
        fn alloc_error(layout: core::alloc::Layout) -> ! {
            simplekernel::logging::raw_put("TEST PANIC: alloc error\n");
            let _ = layout;
            $crate::exit_qemu(1);
        }
    };
}
```

- [ ] **Step 4: 在 workspace `Cargo.toml` 中添加 `test_harness` 为 member**

在 `[workspace] members` 列表中添加 `"crates/test_harness"`。

- [ ] **Step 5: 验证编译**

```bash
cargo check --target riscv64gc-unknown-none-elf -p test_harness
```

Expected: 编译通过（该 crate 是 lib，不生成二进制）。

- [ ] **Step 6: Commit**

```bash
git add crates/test_harness/ Cargo.toml
git commit --signoff -m "feat(test_harness): 创建测试 harness crate，提供 test_main! 宏"
```

---

## Task 2: 迁移 frame_allocator 测试为独立二进制

**Files:**
- Create: `tests/standalone/frame-alloc-test/Cargo.toml`
- Create: `tests/standalone/frame-alloc-test/src/main.rs`
- Create: `tests/standalone/frame-mapped-drop-test/Cargo.toml`
- Create: `tests/standalone/frame-mapped-drop-test/src/main.rs`
- Modify: `Cargo.toml`（workspace members）

### 设计说明

原 7 个测试分为两个二进制：
- `frame-alloc-test`：6 个普通测试（alloc_one_frame、alloc_multiple_frames、alloc_dealloc_realloc、free_into_allocated、full_lifecycle、unmapped_into_allocated）合并为一个二进制依次执行
- `frame-mapped-drop-test`：1 个 should_panic 测试（mapped_drop_panics）单独作为 should_panic 二进制

合并普通测试的原因：这 6 个测试共享同一个初始化级别（`InitLevel::Full`），且它们在独立 QEMU 实例中运行——无需担心互相干扰。每个都单独一个 QEMU 实例太慢（6 次启动 vs 1 次）。

- [ ] **Step 1: 创建 `tests/standalone/frame-alloc-test/Cargo.toml`**

```toml
[package]
name = "frame-alloc-test"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
build = "build.rs"

[[bin]]
name = "frame-alloc-test"
path = "src/main.rs"
test = false
bench = false

[dependencies]
test_harness = { path = "../../../crates/test_harness" }
simplekernel = { path = "../../..", default-features = false }
frame_allocator = { path = "../../../crates/frame_allocator" }
memory_types = { path = "../../../crates/memory_types" }
config = { path = "../../../crates/config" }
log.workspace = true

[target.'cfg(target_arch = "riscv64")'.dependencies]
sbi-rt.workspace = true

[target.'cfg(target_arch = "aarch64")'.dependencies]
aarch64-cpu.workspace = true

[build-dependencies]
cc.workspace = true

[lints]
workspace = true
```

- [ ] **Step 2: 创建 `tests/standalone/frame-alloc-test/build.rs`**

复制 `tests/standalone/panic_test/build.rs`（路径相同：`../../../src/arch`）。

- [ ] **Step 3: 创建 `tests/standalone/frame-alloc-test/src/main.rs`**

```rust
//! 帧分配器测试——验证 typestate 生命周期转换。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_alloc_one_frame();
    log::info!("test alloc_one_frame ... ok");

    test_alloc_multiple_frames();
    log::info!("test alloc_multiple_frames ... ok");

    test_alloc_dealloc_realloc();
    log::info!("test alloc_dealloc_realloc ... ok");

    test_free_into_allocated();
    log::info!("test free_into_allocated ... ok");

    test_full_lifecycle();
    log::info!("test full_lifecycle ... ok");

    test_unmapped_into_allocated();
    log::info!("test unmapped_into_allocated ... ok");

    log::info!("frame_allocator: all 6 tests passed");
}

/// 分配单帧后帧计数应为 1，地址应页对齐。
fn test_alloc_one_frame() {
    let frame = frame_allocator::AllocatedFrames::alloc_one()
        .expect("alloc_one 应成功");
    assert_eq!(frame.count(), 1);
    assert!(frame.start_paddr().is_aligned());
}

/// 分配多帧后帧计数应正确。
fn test_alloc_multiple_frames() {
    let frames = frame_allocator::AllocatedFrames::alloc(4)
        .expect("alloc(4) 应成功");
    assert_eq!(frames.count(), 4);
}

/// 帧 drop 后应能重新分配（归还到分配器）。
fn test_alloc_dealloc_realloc() {
    {
        let _frame = frame_allocator::AllocatedFrames::alloc_one()
            .expect("分配");
    }
    let frame2 = frame_allocator::AllocatedFrames::alloc_one()
        .expect("重新分配应成功");
    assert!(frame2.start_paddr().is_aligned());
}

/// Free -> Allocated 显式转换。
fn test_free_into_allocated() {
    let free = frame_allocator::alloc::alloc_from_buddy(1)
        .expect("buddy 分配");
    let pa = free.start_paddr();
    let allocated = free.into_allocated();
    assert_eq!(allocated.start_paddr(), pa);
    assert_eq!(allocated.count(), 1);
}

/// Allocated -> Mapped -> Unmapped -> Free 完整生命周期。
fn test_full_lifecycle() {
    let free = frame_allocator::alloc::alloc_from_buddy(1)
        .expect("buddy 分配");
    let pa = free.start_paddr();
    let allocated = free.into_allocated();
    let mapped = allocated.into_mapped();
    assert_eq!(mapped.start_paddr(), pa);
    let unmapped = mapped.into_unmapped();
    assert_eq!(unmapped.start_paddr(), pa);
    let _free = unmapped.into_free();
}

/// Unmapped -> Allocated（重新映射路径）。
fn test_unmapped_into_allocated() {
    let free = frame_allocator::alloc::alloc_from_buddy(1)
        .expect("buddy 分配");
    let pa = free.start_paddr();
    let allocated = free.into_allocated();
    let mapped = allocated.into_mapped();
    let unmapped = mapped.into_unmapped();
    let reallocated = unmapped.into_allocated();
    assert_eq!(reallocated.start_paddr(), pa);
}
```

- [ ] **Step 4: 创建 `tests/standalone/frame-mapped-drop-test/` (should_panic)**

`Cargo.toml` 结构同 `frame-alloc-test`，name 改为 `"frame-mapped-drop-test"`。

`src/main.rs`:

```rust
//! MappedFrames drop 应 panic——验证 typestate 安全网。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(
    simplekernel::boot::InitLevel::Full,
    test_mapped_drop_panics,
    should_panic
);

/// MappedFrames 未经 unmap 直接 drop 应触发 panic。
fn test_mapped_drop_panics() {
    let free = frame_allocator::alloc::alloc_from_buddy(1)
        .expect("buddy 分配");
    let _mapped = free.into_allocated().into_mapped();
    // _mapped drop -> panic("Frames<Mapped> dropped without unmap")
}
```

- [ ] **Step 5: 在 workspace `Cargo.toml` 中添加两个新 crate 为 member**

- [ ] **Step 6: 验证交叉编译**

```bash
cargo xtask build --arch riscv64  # 确保新 crate 不破坏构建
```

- [ ] **Step 7: Commit**

```bash
git add tests/standalone/frame-alloc-test/ tests/standalone/frame-mapped-drop-test/ Cargo.toml
git commit --signoff -m "test(frame_allocator): 迁移帧分配器测试为独立 QEMU 二进制"
```

---

## Task 3: 迁移 sync 测试为独立二进制

**Files:**
- Create: `tests/standalone/sync-spinlock-test/Cargo.toml`
- Create: `tests/standalone/sync-spinlock-test/src/main.rs`
- Create: `tests/standalone/sync-lockstack-test/Cargo.toml`
- Create: `tests/standalone/sync-lockstack-test/src/main.rs`
- Create: `tests/standalone/sync-recursive-lock-test/Cargo.toml`
- Create: `tests/standalone/sync-recursive-lock-test/src/main.rs`

### 设计说明

原 15 个 sync 测试分为 3 个二进制：
- `sync-spinlock-test`：SpinLock 基本操作（acquire/release/try_acquire/name/concurrent） — 合并系统测试中已有的 3 个 + ttas.rs 中的 6 个普通测试
- `sync-lockstack-test`：锁栈级别检查 — lock_stack.rs 中的 5 个普通测试
- `sync-recursive-lock-test`：递归锁检测 (should_panic) — `check_recursive_panics_when_same_core`
- `sync-lockstack-pop-mismatch-test`：锁栈 pop 不匹配 (should_panic) — `pop_mismatch_panics`

注意：`concurrent_acquire_release` 测试在裸机单核上无法直接测多线程。需要改写为使用内核任务（`simplekernel::task::spawn`）模拟并发，或删除（由系统测试的多核 SMP 覆盖）。本计划中暂不迁移该测试，标记为 TODO。

- [ ] **Step 1-4: 创建各测试二进制**

每个二进制的结构同 Task 2。`sync-spinlock-test` 包含原 ttas.rs 中 `new_lock_is_unlocked`、`acquire_sets_locked`、`release_clears_locked`、`try_acquire_succeeds_when_free`、`try_acquire_fails_when_held`、`name_returns_given_name`，加上原系统测试的 `spinlock_basic`、`spinlock_modify`、`spinlock_not_held_after_drop` 及 `check_recursive_ok_after_clear_owner`。

`sync-lockstack-test` 包含 `empty_stack_allows_any_level`、`unspecified_skips_order_check`、`unspecified_on_top_skips_check`、`normal_levels_must_increase`、`push_pop_roundtrip`。

`sync-recursive-lock-test` 和 `sync-lockstack-pop-mismatch-test` 使用 `should_panic` 变体。

注意：ttas.rs 中的测试直接操作 `RawSpinLock` 的 `acquire`/`release`/`set_owner`/`clear_owner` 方法，这些是 `pub(crate)`。独立测试二进制无法访问。需要改为通过 `SpinLock` 公共 API 测试（`lock()`/`try_lock()`/`is_locked()`），或在 sync crate 中暴露 `#[doc(hidden)] pub` 测试辅助接口。

**推荐做法**：通过 `SpinLock` 公共 API 重写测试。原 `RawSpinLock` 级别的测试是实现细节验证，不需要在集成测试中重复——`SpinLock` 公共 API 测试已覆盖核心行为。

- [ ] **Step 5: Commit**

```bash
git commit --signoff -m "test(sync): 迁移同步原语测试为独立 QEMU 二进制"
```

---

## Task 4: 迁移 paging 测试为独立二进制

**Files:**
- Create: `tests/standalone/paging-basic-test/Cargo.toml`
- Create: `tests/standalone/paging-basic-test/src/main.rs`
- Create: `tests/standalone/paging-table-test/Cargo.toml`
- Create: `tests/standalone/paging-table-test/src/main.rs`
- Create: `tests/standalone/paging-mapping-test/Cargo.toml`
- Create: `tests/standalone/paging-mapping-test/src/main.rs`

### 设计说明

原 38 个 paging 测试分为 3 个二进制：
- `paging-basic-test`：lib.rs 中 4 个参数计算测试
- `paging-table-test`：table.rs 中 29 个页表操作测试
- `paging-mapping-test`：mapping.rs 中 5 个 MappedPages 生命周期测试

所有测试使用 `InitLevel::Full`（需要帧分配器 + 页表子系统完整初始化）。

- [ ] **Step 1-3: 创建各测试二进制**

结构同 Task 2。测试函数从原 `#[cfg(test)] mod tests` 中提取，去掉 `ensure_test_init()` 调用（`kernel_init` 已完成初始化）。

注意：原测试中使用 `alloc_from_buddy()` 分配帧再手动转换状态。在裸机环境中可以直接使用 `AllocatedFrames::alloc()` / `alloc_one()`——`to_virt()` 在裸机下正常工作。

- [ ] **Step 4: Commit**

```bash
git commit --signoff -m "test(paging): 迁移页表测试为独立 QEMU 二进制"
```

---

## Task 5: 迁移 memory/vma 和现有系统测试为独立二进制

**Files:**
- Create: `tests/standalone/vma-test/Cargo.toml`
- Create: `tests/standalone/vma-test/src/main.rs`
- Create: `tests/standalone/heap-test/Cargo.toml`
- Create: `tests/standalone/heap-test/src/main.rs`
- Create: `tests/standalone/device-test/Cargo.toml`
- Create: `tests/standalone/device-test/src/main.rs`
- Create: `tests/standalone/fs-test/Cargo.toml`
- Create: `tests/standalone/fs-test/src/main.rs`

### 设计说明

- `vma-test`：memory/vma.rs 中 8 个 VMA 测试
- `heap-test`：原 system-test 中 3 个堆分配测试
- `device-test`：原 system-test 中 3 个设备测试
- `fs-test`：原 system-test 中 5 个文件系统测试

- [ ] **Step 1-4: 创建各测试二进制**
- [ ] **Step 5: Commit**

```bash
git commit --signoff -m "test(memory/device/fs): 迁移 VMA/堆/设备/文件系统测试为独立二进制"
```

---

## Task 6: 升级 xtask 支持并行 QEMU 和 should_panic 检测

**Files:**
- Modify: `xtask/src/test.rs`
- Modify: `xtask/src/main.rs`
- Modify: `xtask/src/qemu.rs`

### 设计说明

**并行执行**：`xtask test --all` 发现所有 `tests/standalone/` 下的测试包，用线程池（`std::thread`）并行启动 QEMU 实例。默认并行度 = `std::thread::available_parallelism()`，可通过 `--jobs N` 覆盖。

**should_panic 检测**：在测试包的 `Cargo.toml` 中添加 metadata 标记：
```toml
[package.metadata.test]
should_panic = "expected panic message"
```
`xtask` 读取此 metadata。对于 `should_panic` 测试，判定逻辑改为：QEMU 退出码 0 = 成功（panic handler 中调用了 `exit_qemu(0)`）。

**输出捕获**：`launch_qemu` 需要改为捕获 stdout/stderr（当前直接 `-serial stdio` 透传）。用 `std::process::Command` 的 `stdout(Stdio::piped())` 捕获，测试结束后检查输出。

- [ ] **Step 1: 修改 `xtask/src/qemu.rs`——添加输出捕获版本的 `launch_qemu_captured`**

添加新函数 `launch_qemu_captured`，返回 `Result<(bool, String)>`（成功/失败 + 串口输出）。

```rust
/// 启动 QEMU 并捕获串口输出，返回 (QEMU 退出是否成功, 串口输出)。
pub fn launch_qemu_captured(
    sh: &Shell,
    arch: Arch,
    project_root: &Path,
    boot_dir: &Path,
    kernel_elf_path: &Path,
    rootfs_path: &Path,
    timeout_secs: u64,
) -> Result<(bool, String)> {
    // 构建 QEMU 命令但不通过 xshell 运行——用 std::process::Command 捕获输出
    // ... 构建参数（复用 base_qemu_cmd 的参数逻辑）
    // ... 设置 stdout/stderr piped
    // ... 启动进程，等待 timeout_secs 后 kill
    // ... 返回 (exit_code == 0, stdout_string)
}
```

- [ ] **Step 2: 修改 `xtask/src/test.rs`——并行执行 + should_panic**

```rust
use std::sync::mpsc;
use std::thread;

/// 测试元数据
struct TestMeta {
    name: String,
    should_panic: Option<String>,  // None = 普通测试, Some(msg) = should_panic
}

/// 从 Cargo.toml 读取测试元数据
fn read_test_meta(cargo_toml: &Path) -> Option<TestMeta> {
    let content = std::fs::read_to_string(cargo_toml).ok()?;
    let name = read_package_name(cargo_toml)?;
    // 解析 [package.metadata.test] should_panic = "..."
    let should_panic = parse_should_panic_meta(&content);
    Some(TestMeta { name, should_panic })
}

/// 并行运行所有独立测试
pub fn run_all_standalone_parallel(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    env: &QemuEnv,
    release: bool,
    jobs: usize,
) -> Result<bool> {
    let tests = collect_all_test_metas(project_root);
    let (tx, rx) = mpsc::channel();

    // 线程池并行执行
    let semaphore = std::sync::Arc::new(std::sync::Semaphore::new(jobs));
    for test in &tests {
        let tx = tx.clone();
        let sem = semaphore.clone();
        // ... spawn thread: 获取信号量 → build → launch_qemu_captured → 判定 → 发送结果
    }
    drop(tx);

    // 收集结果
    let mut all_passed = true;
    for result in rx {
        if !result.passed {
            all_passed = false;
            eprintln!("[xtask] FAIL: {}", result.name);
            eprintln!("{}", result.output);  // 打印失败测试的完整输出
        } else {
            println!("[xtask] PASS: {}", result.name);
        }
    }
    Ok(all_passed)
}
```

注意：`std::sync::Semaphore` 在标准库中不存在，需要用 `Arc<Mutex<usize>>` + `Condvar` 或简单的 channel-based 信号量实现。或者使用 `rayon` 的并行迭代器。检查 xtask 的 Cargo.toml 是否已有 rayon 依赖，如果没有，用手写线程池。

- [ ] **Step 3: 修改 `xtask/src/main.rs`——添加 `--jobs` 和 `--filter` 参数**

```rust
#[derive(Args, Clone)]
struct TestArgs {
    #[arg(long, value_enum, default_value = "riscv64")]
    arch: Arch,
    #[arg(long)]
    release: bool,
    /// 运行指定的独立测试（支持子串匹配）
    #[arg(long)]
    name: Option<String>,
    /// 运行全部测试
    #[arg(long)]
    all: bool,
    /// 列出可用测试
    #[arg(long)]
    list: bool,
    /// 并行 QEMU 实例数（默认 = CPU 核心数）
    #[arg(long, short = 'j')]
    jobs: Option<usize>,
}
```

- [ ] **Step 4: Commit**

```bash
git commit --signoff -m "feat(xtask): 支持并行 QEMU 测试执行和 should_panic 检测"
```

---

## Task 7: （已合并到 Task 0）

冒烟测试已在 Task 0 中合入 `boot.rs::kernel_init()`，此 task 无额外工作。

---

## Task 8: 删除宿主机测试代码和 host mock

**Files:**
- Modify: 见"删除文件/代码"表

### 设计说明

此 task 在所有测试迁移完成且 QEMU 测试通过后执行。逐个 crate 删除。

- [ ] **Step 1: frame_allocator——删除 `ensure_test_init`、`test-support`、`cfg_attr`、test 模块**

`src/lib.rs`: 删除 `#![cfg_attr(not(any(test, feature = "test-support")), no_std)]` 改为 `#![no_std]`；删除 `ensure_test_init()` 函数。
`src/transitions.rs`: 删除 `#[cfg(test)] mod tests` 整个模块。
`Cargo.toml`: 删除 `[features]` 段中的 `test-support = []`。

- [ ] **Step 2: paging——删除 `ensure_test_init`、`test-support`、`dev-dependencies`、所有 test 模块**

`src/lib.rs`: 删除 `ensure_test_init()` 和 `#[cfg(test)] mod tests`。
`src/table.rs`: 删除 `#[cfg(test)] mod tests`。
`src/mapping.rs`: 删除 `#[cfg(test)] mod tests`。
`Cargo.toml`: 删除 `[features]` 中 `test-support`、删除 `[dev-dependencies]`。

- [ ] **Step 3: sync——删除 `cfg_attr`、`caller_id` 双路径、所有 test 模块**

`src/lib.rs`: `#![cfg_attr(not(test), no_std)]` → `#![no_std]`。
`src/raw/ttas.rs`: 删除 `#[cfg(test)]` / `#[cfg(not(test))]` 双路径，只保留 `per_cpu::current_core_id()` 路径；删除 `#[cfg(test)] mod tests`。
`src/lock_stack.rs`: 删除 `#[cfg(test)] mod tests`。

- [ ] **Step 4: memory——删除 `dev-dependencies`、vma test 模块**

`src/vma.rs`: 删除 `#[cfg(test)] mod tests`。
`Cargo.toml`: 删除 `[dev-dependencies]`。

- [ ] **Step 5: per_cpu——删除 host.rs 和 cfg 分支**

删除 `src/host.rs` 文件。
`src/lib.rs`: 删除 `#[cfg(not(bare_metal))] mod host` 及 `pub use host::*`。

注意：`arch` crate 的 `host.rs` **不删除**——它为纯逻辑 crate 提供编译支持（ADR-004 明确保留）。

- [ ] **Step 6: local_tick、src/panic.rs——删除 cfg_attr 和 test mock**

`crates/local_tick/src/lib.rs`: `#![cfg_attr(not(test), no_std)]` → `#![no_std]`。
`src/panic.rs`: 删除 `#[cfg(test)]` 的 `handle_panic` mock、`raw_dump_stack` mock 和 `mod tests`。

- [ ] **Step 7: 验证裸机构建**

```bash
cargo xtask build --arch riscv64
cargo xtask build --arch aarch64
```

- [ ] **Step 8: Commit**

```bash
git commit --signoff -m "refactor: 删除内核 crate 宿主机测试代码和 host mock（ADR-004）"
```

---

## Task 9: 删除统一系统测试、更新 CI

**Files:**
- Delete: `tests/system/` 目录
- Modify: `.github/workflows/workflow.yml`
- Modify: `Cargo.toml`（移除 system-test member）

- [ ] **Step 1: 删除 `tests/system/` 目录**

```bash
rm -rf tests/system/
```

- [ ] **Step 2: 更新 CI workflow**

`.github/workflows/workflow.yml` 中：

**Unit tests (host)** 改为只跑纯逻辑 crate：
```yaml
- name: Unit tests (host)
  run: cargo test -p memory_types -p config -p page_table_entry -p span -p arch
```

**System Test** 改为：
```yaml
- name: System Test
  run: |
    for i in $(seq 1 $SYSTEM_TEST_RUNS); do
      echo "=== ${{ matrix.arch }} System Test Run $i/$SYSTEM_TEST_RUNS ==="
      timeout 600 cargo xtask test --arch ${{ matrix.arch }} --all -j 4
      echo "${{ matrix.arch }} system test run $i/$SYSTEM_TEST_RUNS passed"
    done
```

注意超时从 300 增加到 600（并行跑所有独立二进制，总时间可能变长）。

- [ ] **Step 3: 更新 workspace `Cargo.toml`——移除 system-test member**

- [ ] **Step 4: 验证 CI 流程本地模拟**

```bash
# 纯逻辑测试
cargo test -p memory_types -p config -p page_table_entry -p span -p arch

# 全量 QEMU 测试
cargo xtask test --arch riscv64 --all -j 4
```

- [ ] **Step 5: Commit**

```bash
git commit --signoff -m "chore: 删除统一系统测试，更新 CI 为纯逻辑 + 并行 QEMU 测试"
```

---

## Task 10: 更新文档

**Files:**
- Modify: `CLAUDE.md`（TESTING 章节）

- [ ] **Step 1: 更新 CLAUDE.md 的 TESTING 章节**

反映新的两层测试体系：
- 纯逻辑 crate：`cargo test -p memory_types -p config -p page_table_entry -p span -p arch`
- QEMU 独立测试：`cargo xtask test --arch riscv64 --all`
- 冒烟测试：内核启动时自动运行
- 添加独立测试的方法

- [ ] **Step 2: Commit**

```bash
git commit --signoff -m "docs: 更新测试文档，反映独立 QEMU 测试 + 冒烟测试体系"
```
