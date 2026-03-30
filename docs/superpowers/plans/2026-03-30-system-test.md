# 系统测试框架实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 SimpleKernel 添加 QEMU 环境下的系统测试框架，支持统一测试内核和独立二进制测试。

**Architecture:** 将内核 crate 拆分为 `lib.rs` + `main.rs`，暴露 `kernel_init()` 分级初始化接口。测试代码作为独立 crate 依赖内核 lib，替换入口点。`xtask` 新增 `test` 子命令编排 QEMU 测试。

**Tech Stack:** Rust nightly (`no_std`), `qemu-exit` crate, QEMU, `xshell` + `clap` (xtask)

**Spec:** `docs/superpowers/specs/2026-03-30-system-test-design.md`

---

## 文件结构

### 新建文件

| 文件 | 职责 |
|------|------|
| `src/lib.rs` | 内核库入口，re-export 所有模块 + `kernel_init()` + `InitLevel` |
| `src/boot.rs` | 启动序列逻辑（`kernel_init` 实现），从 `main.rs` 提取 |
| `tests/system/Cargo.toml` | 统一测试内核 crate 配置 |
| `tests/system/build.rs` | 引用内核 linker script |
| `tests/system/src/main.rs` | 测试入口：`_start` → `kernel_init(Full)` → runner → qemu_exit |
| `tests/system/src/framework.rs` | TestRunner, TestCase, TestGroup, 输出格式化 |
| `tests/system/src/memory.rs` | 内存测试组 |
| `tests/system/src/sync_tests.rs` | 同步原语测试组 |
| `tests/system/src/smp.rs` | SMP 测试组 |
| `tests/system/src/task_tests.rs` | 任务测试组 |
| `tests/standalone/panic_test/Cargo.toml` | panic 行为独立测试 |
| `tests/standalone/panic_test/build.rs` | 引用内核 linker script |
| `tests/standalone/panic_test/src/main.rs` | panic 测试入口 |
| `xtask/src/test.rs` | `cargo xtask test` 子命令实现 |

### 修改文件

| 文件 | 修改内容 |
|------|----------|
| `src/main.rs` | 移除模块声明（移入 `lib.rs`），仅保留 `_start` 和引导函数 |
| `Cargo.toml` | workspace members 新增测试 crate |
| `build.rs` | 无修改（测试 crate 的 build.rs 独立引用内核 linker script） |
| `xtask/src/main.rs` | 新增 `Test` 子命令 |
| `xtask/Cargo.toml` | 无新依赖（已有 `clap` + `xshell`） |
| `.github/workflows/workflow.yml` | 替换 grep 逻辑为 `cargo xtask test` |

---

## Task 1: 内核 crate 拆分为 lib + bin

将 `src/main.rs` 中的模块声明和公共类型移入 `src/lib.rs`，`main.rs` 仅保留入口。

**Files:**
- Create: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: 创建 `src/lib.rs`，迁移模块声明**

```rust
// src/lib.rs
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), feature(alloc_error_handler))]
#![feature(sync_unsafe_cell)]
#![cfg_attr(test, allow(dead_code))]

extern crate alloc;

/// 实际在线核心数（从 FDT 解析，`early_init` 中初始化）。
pub static CORE_COUNT: spin::Once<usize> = spin::Once::new();

#[cfg(not(test))]
pub mod arch;
pub mod elf;
#[cfg(not(test))]
pub mod fdt;
#[cfg(not(test))]
pub mod init;
pub mod irq_context;
#[cfg(not(test))]
pub mod lang_items;
pub mod logging;
pub mod panic;
pub mod preempt;
pub mod syscall;
pub mod task;
#[cfg(not(test))]
pub mod timer;
pub mod util;
```

注意：所有 `mod` 从 `pub(crate)` 改为 `pub`，以便测试 crate 能访问。

- [ ] **Step 2: 精简 `src/main.rs`，仅保留入口**

```rust
// src/main.rs
#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};
use simplekernel::arch::{Arch, ArchOps};
use simplekernel::*;

mod smoke_test;

static PRIMARY_BOOTED: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
    if !PRIMARY_BOOTED.swap(true, Ordering::AcqRel) {
        bootstrap(argc, argv);
    } else {
        bootstrap_smp(argc, argv);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(entry: usize, arg: usize) -> ! {
    unsafe { task::bootstrap_enable_irq() };
    let entry_fn: fn(usize) = unsafe { core::mem::transmute(entry) };
    entry_fn(arg);
    task::exit(0);
}

fn bootstrap(argc: i32, argv: *const *const u8) -> ! {
    logging::init();
    unsafe { per_cpu::percpu_init() };
    init::early_init(Arch::dtb_addr(argc, argv));
    smoke_test::phase2();
    let mut kernel_as = memory::init();
    Arch::map_early_mmio(&mut kernel_as).expect("failed to map early MMIO");
    {
        let pt = kernel_as.page_table().lock();
        unsafe { Arch::activate_page_table(&pt) };
    }
    log::info!("MemoryInit: paging enabled");
    memory::store_kernel_address_space(kernel_as);
    smoke_test::phase3();
    Arch::init_timer();
    Arch::init_interrupt();
    task::init();
    Arch::wake_secondary_cores();
    smoke_test::phase4();
    smoke_test::spawn_all();
    task::schedule();
    loop {
        if preempt::check_and_clear_need_resched() {
            task::schedule();
        }
        core::hint::spin_loop();
    }
}

fn bootstrap_smp(_argc: i32, _argv: *const *const u8) -> ! {
    unsafe { per_cpu::percpu_init_smp() };
    let core_id = per_cpu::current_core_id();
    memory::init_smp(|pt| {
        unsafe { Arch::activate_page_table(pt) };
    });
    task::init_smp();
    Arch::init_timer_smp(core_id);
    Arch::init_interrupt_smp();
    log::info!("SMP: core {} online", core_id);
    task::schedule();
    loop {
        if preempt::check_and_clear_need_resched() {
            task::schedule();
        }
        core::hint::spin_loop();
    }
}
```

**重要**：`main.rs` 中不再重复 `#![cfg_attr(not(test), no_std)]` 等属性——这些在 `lib.rs` 中定义。`main.rs` 用 `#![no_std]` + `#![no_main]`（无条件，因为 main.rs 不参与 host 测试）。

`smoke_test` 保持为 `main.rs` 的私有模块（`mod smoke_test;`），因为它是内核运行时专用的，不需要暴露给测试 crate。

- [ ] **Step 3: 验证编译**

```bash
cargo build --target riscv64gc-unknown-none-elf -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem
```

预期：编译成功，无新 warning。

如果 `main.rs` 中引用 `lib.rs` 的模块出现可见性问题，逐个调整 `pub` / `pub(crate)` 直到编译通过。关键是：测试 crate 需要 `pub` 的模块有 `arch`、`logging`、`init`、`memory`、`task`、`preempt`、`per_cpu`（来自外部 crate）。

- [ ] **Step 4: 验证 host 单元测试**

```bash
cargo test
```

预期：所有现有单元测试通过。

- [ ] **Step 5: 提交**

```bash
git add src/lib.rs src/main.rs
git commit --signoff -m "refactor: 拆分内核 crate 为 lib + bin

将模块声明移入 src/lib.rs，main.rs 仅保留入口函数。
为系统测试 crate 依赖内核模块做准备。"
```

---

## Task 2: 添加 `kernel_init()` 分级初始化接口

从 `bootstrap()` 中提取初始化逻辑到 `src/boot.rs`，暴露 `kernel_init()`。

**Files:**
- Create: `src/boot.rs`
- Modify: `src/lib.rs` (新增 `pub mod boot;`)
- Modify: `src/main.rs` (调用 `boot::kernel_init()`)

- [ ] **Step 1: 创建 `src/boot.rs`**

```rust
//! 内核分级初始化接口。
//!
//! 提供 `kernel_init()` 函数，将启动序列分解为独立级别，
//! 供内核主入口和系统测试共用。

use crate::arch::{Arch, ArchOps};

/// 内核初始化级别
pub enum InitLevel {
    /// 日志 + per_cpu + early_init + 内存子系统 + 页表激活
    Memory,
    /// Memory + 定时器 + 中断控制器
    Interrupt,
    /// Interrupt + 任务子系统 + SMP 唤醒（完整初始化）
    Full,
}

/// 初始化内核子系统到指定级别。
///
/// # Safety
/// - 必须在 bare-metal 环境调用
/// - 每个级别只能调用一次
/// - 调用前必须已设置好栈和 per-CPU 基础寄存器（由汇编入口完成）
pub unsafe fn kernel_init(argc: i32, argv: *const *const u8, level: InitLevel) {
    // ── Level: Memory ──
    crate::logging::init();
    unsafe { per_cpu::percpu_init() };
    crate::init::early_init(Arch::dtb_addr(argc, argv));

    let mut kernel_as = memory::init();
    Arch::map_early_mmio(&mut kernel_as).expect("failed to map early MMIO");
    {
        let pt = kernel_as.page_table().lock();
        unsafe { Arch::activate_page_table(&pt) };
    }
    log::info!("MemoryInit: paging enabled");
    memory::store_kernel_address_space(kernel_as);

    if matches!(level, InitLevel::Memory) {
        return;
    }

    // ── Level: Interrupt ──
    Arch::init_timer();
    Arch::init_interrupt();

    if matches!(level, InitLevel::Interrupt) {
        return;
    }

    // ── Level: Full ──
    crate::task::init();
    Arch::wake_secondary_cores();
}

/// 从核初始化序列（由从核入口调用）。
///
/// # Safety
/// 必须在从核的汇编入口跳转后调用，per-CPU 寄存器已设置。
pub unsafe fn kernel_init_smp() {
    unsafe { per_cpu::percpu_init_smp() };
    let core_id = per_cpu::current_core_id();
    memory::init_smp(|pt| {
        unsafe { Arch::activate_page_table(pt) };
    });
    crate::task::init_smp();
    Arch::init_timer_smp(core_id);
    Arch::init_interrupt_smp();
    log::info!("SMP: core {} online", core_id);
}
```

- [ ] **Step 2: 在 `lib.rs` 中注册模块**

在 `src/lib.rs` 的模块列表中添加：

```rust
#[cfg(not(test))]
pub mod boot;
```

- [ ] **Step 3: 修改 `main.rs` 的 `bootstrap_smp()` 使用 `kernel_init_smp()`**

`main.rs` 的 `bootstrap()` **保持不变**——它需要在初始化步骤之间穿插烟雾测试（`phase2()`/`phase3()`/`phase4()`），而 `kernel_init()` 是一口气完成初始化的，仅供测试 crate 使用。

仅将 `bootstrap_smp()` 改用 `boot::kernel_init_smp()` 消除重复：

```rust
fn bootstrap_smp(_argc: i32, _argv: *const *const u8) -> ! {
    // SAFETY: 从核入口，汇编已设置栈和寄存器
    unsafe { boot::kernel_init_smp() };
    task::schedule();
    loop {
        if preempt::check_and_clear_need_resched() {
            task::schedule();
        }
        core::hint::spin_loop();
    }
}
```

- [ ] **Step 4: 验证编译（双架构）**

```bash
cargo build --target riscv64gc-unknown-none-elf -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem
cargo build --target aarch64-unknown-none-softfloat -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem
```

预期：编译成功。

- [ ] **Step 5: 提交**

```bash
git add src/boot.rs src/lib.rs src/main.rs
git commit --signoff -m "feat(boot): 添加 kernel_init() 分级初始化接口

提取启动序列到 boot.rs，暴露 InitLevel::Memory/Interrupt/Full 三级初始化。
测试 crate 可通过此接口启动内核子系统而无需复刻 bootstrap 流程。"
```

---

## Task 3: 创建测试框架模块

实现 `TestRunner`、`TestCase`、`TestGroup` 等核心类型和 `cargo test` 风格输出。

**Files:**
- Create: `tests/system/src/framework.rs`

- [ ] **Step 1: 创建 `tests/system/` 目录结构**

```bash
mkdir -p tests/system/src
```

- [ ] **Step 2: 编写 `tests/system/src/framework.rs`**

```rust
//! 系统测试框架——轻量 TestRunner，cargo test 风格输出。

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

/// 单个测试用例
pub struct TestCase {
    pub name: &'static str,
    pub run: fn(),
}

/// 顺序测试组
pub struct TestGroup {
    pub name: &'static str,
    pub tests: &'static [TestCase],
}

/// 并发测试线程定义
pub struct ThreadTestCase {
    pub name: &'static str,
    pub run: fn(usize),
}

/// 并发测试组
pub struct ConcurrentTestGroup {
    pub name: &'static str,
    pub threads: &'static [ThreadTestCase],
    pub thread_count: usize,
    pub verify: fn(),
}

/// 全局标志：当前是否在测试执行中（panic handler 据此判断行为）
pub static IN_TEST: AtomicBool = AtomicBool::new(false);

/// 当前正在执行的测试名（panic 时记录）
pub static CURRENT_TEST_FAILED: AtomicBool = AtomicBool::new(false);

/// 测试运行器
pub struct TestRunner {
    groups: Vec<TestGroup>,
    concurrent_groups: Vec<ConcurrentTestGroup>,
    total_passed: usize,
    total_failed: usize,
    failures: Vec<(&'static str, &'static str)>,
}

impl TestRunner {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            concurrent_groups: Vec::new(),
            total_passed: 0,
            total_failed: 0,
            failures: Vec::new(),
        }
    }

    pub fn add_group(&mut self, group: TestGroup) {
        self.groups.push(group);
    }

    pub fn add_concurrent_group(&mut self, group: ConcurrentTestGroup) {
        self.concurrent_groups.push(group);
    }

    /// 运行所有测试组，返回是否全部通过
    pub fn run(&mut self) -> bool {
        log::info!("=== SimpleKernel System Tests ===");
        log::info!("");

        // 顺序测试组
        for group in &self.groups {
            self.run_sequential_group(group);
        }

        // 并发测试组
        for group in &self.concurrent_groups {
            self.run_concurrent_group(group);
        }

        // 最终摘要
        log::info!("================================");
        if self.total_failed == 0 {
            log::info!(
                "test result: ok. {} passed; 0 failed",
                self.total_passed
            );
        } else {
            log::error!(
                "test result: FAILED. {} passed; {} failed",
                self.total_passed,
                self.total_failed
            );
            log::error!("failures:");
            for (group, name) in &self.failures {
                log::error!("    {}::{}", group, name);
            }
        }

        self.total_failed == 0
    }

    fn run_sequential_group(&mut self, group: &TestGroup) {
        let count = group.tests.len();
        log::info!("running {} tests in group \"{}\"", count, group.name);

        let mut passed = 0usize;
        let mut failed = 0usize;

        for test in group.tests {
            IN_TEST.store(true, Ordering::Release);
            CURRENT_TEST_FAILED.store(false, Ordering::Release);

            // 运行测试——如果 panic，panic handler 会设置 CURRENT_TEST_FAILED
            // 并终止当前组（longjmp 回到 runner 不在 MVP 中实现，
            // panic 会直接终止当前组剩余测试）
            (test.run)();

            IN_TEST.store(false, Ordering::Release);

            if CURRENT_TEST_FAILED.load(Ordering::Acquire) {
                log::error!("test {} ... FAILED", test.name);
                failed += 1;
                self.failures.push((group.name, test.name));
            } else {
                log::info!("test {} ... ok", test.name);
                passed += 1;
            }
        }

        if failed == 0 {
            log::info!("group result: ok. {} passed; 0 failed", passed);
        } else {
            log::error!(
                "group result: FAILED. {} passed; {} failed",
                passed,
                failed
            );
        }
        log::info!("");

        self.total_passed += passed;
        self.total_failed += failed;
    }

    fn run_concurrent_group(&mut self, group: &ConcurrentTestGroup) {
        log::info!(
            "running {} tests in group \"{}\" (concurrent, {} threads each)",
            group.threads.len(),
            group.name,
            group.thread_count
        );

        IN_TEST.store(true, Ordering::Release);
        CURRENT_TEST_FAILED.store(false, Ordering::Release);

        // 为每个线程测试 spawn 内核线程
        for thread_test in group.threads {
            for i in 0..group.thread_count {
                simplekernel::task::spawn_kernel_thread(
                    thread_test.name,
                    thread_test.run,
                    i,
                );
            }
        }

        // 让出 CPU 让测试线程运行
        for _ in 0..1000 {
            simplekernel::task::yield_now();
        }

        // 调用验证函数
        (group.verify)();

        IN_TEST.store(false, Ordering::Release);

        if CURRENT_TEST_FAILED.load(Ordering::Acquire) {
            log::error!("group \"{}\" ... FAILED", group.name);
            self.total_failed += 1;
            self.failures.push((group.name, "concurrent"));
        } else {
            log::info!("group \"{}\" ... ok", group.name);
            self.total_passed += 1;
        }
        log::info!("");
    }
}
```

- [ ] **Step 3: 提交**

```bash
git add tests/system/src/framework.rs
git commit --signoff -m "feat(test): 添加系统测试框架 TestRunner

实现 TestCase/TestGroup/ConcurrentTestGroup 类型和 cargo-test 风格输出。
支持顺序和并发两种测试组执行模式。"
```

---

## Task 4: 创建统一测试内核 crate

配置 `Cargo.toml`、`build.rs`、`main.rs`，使测试 crate 能在 QEMU 中启动。

**Files:**
- Create: `tests/system/Cargo.toml`
- Create: `tests/system/build.rs`
- Create: `tests/system/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: 创建 `tests/system/Cargo.toml`**

```toml
[package]
name = "system-test"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
build = "build.rs"

[dependencies]
simplekernel = { path = "../.." }

# 需要直接依赖的内核子 crate（用于测试其公共 API）
memory = { path = "../../crates/memory" }
sync = { path = "../../crates/sync" }
per_cpu = { path = "../../crates/per_cpu" }
config = { path = "../../crates/config" }
address = { path = "../../crates/address" }
global_tick = { path = "../../crates/global_tick" }

# 外部依赖
log.workspace = true
qemu-exit.workspace = true
spin.workspace = true

# RISC-V
[target.'cfg(target_arch = "riscv64")'.dependencies]
sbi-rt.workspace = true

# AArch64
[target.'cfg(target_arch = "aarch64")'.dependencies]
aarch64-cpu.workspace = true

[build-dependencies]
cc.workspace = true
```

- [ ] **Step 2: 创建 `tests/system/build.rs`**

复用内核的汇编编译和 linker script，不复制文件：

```rust
//! 测试内核构建脚本——复用内核的汇编入口和 linker script。

use std::path::PathBuf;

fn compiler_for(arch: &str) -> &'static str {
    match arch {
        "riscv64" => "riscv64-linux-gnu-gcc",
        "aarch64" => "aarch64-linux-gnu-gcc",
        _ => unreachable!("unsupported architecture: {arch}"),
    }
}

fn extra_flags_for(arch: &str) -> &'static [&'static str] {
    match arch {
        "riscv64" => &["-march=rv64gc", "-mabi=lp64d"],
        _ => &[],
    }
}

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if !matches!(arch.as_str(), "riscv64" | "aarch64") || os != "none" {
        return;
    }

    // 内核源码中的架构目录（相对于 workspace root）
    let kernel_arch_dir = PathBuf::from("../../src/arch").join(&arch);

    // ── 编译汇编文件（与内核 build.rs 相同） ───────────────────────
    let mut build = cc::Build::new();
    build.compiler(compiler_for(&arch));

    for flag in extra_flags_for(&arch) {
        build.flag(flag);
    }

    build.include(&kernel_arch_dir);

    let mut has_asm = false;
    for entry in std::fs::read_dir(&kernel_arch_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", kernel_arch_dir.display()))
    {
        let path = entry
            .unwrap_or_else(|e| panic!("cannot read entry in {}: {e}", kernel_arch_dir.display()))
            .path();
        if path.extension().is_some_and(|ext| ext == "S") {
            build.file(&path);
            has_asm = true;
        }
    }

    if has_asm {
        build.compile("asm");
    }

    // ── 链接器参数（引用内核的 linker script） ──────────────────────
    println!("cargo:rustc-link-arg=-z");
    println!("cargo:rustc-link-arg=norelro");
    println!(
        "cargo:rustc-link-arg=-T{}",
        kernel_arch_dir.join("link.ld").display()
    );

    println!("cargo:rerun-if-changed={}", kernel_arch_dir.display());
}
```

- [ ] **Step 3: 创建 `tests/system/src/main.rs`**

```rust
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

mod framework;
mod memory;
mod sync_tests;

use core::sync::atomic::{AtomicBool, Ordering};
use framework::{TestRunner, TestGroup};

static PRIMARY_BOOTED: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
    if !PRIMARY_BOOTED.swap(true, Ordering::AcqRel) {
        test_main(argc, argv);
    } else {
        test_smp(argc, argv);
    }
}

/// 内核线程引导函数（switch.S 中 kernel_thread_entry 需要此符号）
#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(entry: usize, arg: usize) -> ! {
    unsafe { simplekernel::task::bootstrap_enable_irq() };
    let entry_fn: fn(usize) = unsafe { core::mem::transmute(entry) };
    entry_fn(arg);
    simplekernel::task::exit(0);
}

fn test_main(argc: i32, argv: *const *const u8) -> ! {
    // 完整初始化内核
    // SAFETY: 主核首次调用，汇编入口已设置栈和寄存器
    unsafe {
        simplekernel::boot::kernel_init(
            argc,
            argv,
            simplekernel::boot::InitLevel::Full,
        );
    }

    // 构建并运行测试
    let mut runner = TestRunner::new();

    runner.add_group(TestGroup {
        name: "memory",
        tests: memory::tests(),
    });
    runner.add_group(TestGroup {
        name: "sync",
        tests: sync_tests::tests(),
    });

    let all_passed = runner.run();

    // 通过 qemu-exit 退出
    exit_qemu(if all_passed { 0 } else { 1 });
}

fn test_smp(_argc: i32, _argv: *const *const u8) -> ! {
    // SAFETY: 从核入口
    unsafe { simplekernel::boot::kernel_init_smp() };
    simplekernel::task::schedule();
    loop {
        if simplekernel::preempt::check_and_clear_need_resched() {
            simplekernel::task::schedule();
        }
        core::hint::spin_loop();
    }
}

fn exit_qemu(code: u32) -> ! {
    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V: 使用 SBI shutdown
        sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
        loop { core::hint::spin_loop(); }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // AArch64: 使用 QEMU virt 的 sysreg exit
        use qemu_exit::QEMUExit;
        let qemu_exit = qemu_exit::AArch64::new();
        qemu_exit.exit(code);
    }
}

// ── lang items ──────────────────────────────────────────────────────

use core::alloc::Layout;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    // 如果在测试执行中 panic，标记测试失败
    if framework::IN_TEST.load(Ordering::Acquire) {
        framework::CURRENT_TEST_FAILED.store(true, Ordering::Release);
        simplekernel::logging::raw_put("\x1b[31m[TEST PANIC]\x1b[0m ");
        // 打印 panic 信息后继续（注意：实际上 panic handler 是 diverging 的，
        // 这里无法真正 "继续"。MVP 方案：panic 直接终止测试并通过 qemu-exit 退出）
    }
    simplekernel::panic::handle_panic(info);
}

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    simplekernel::logging::raw_put("TEST PANIC: alloc error\n");
    exit_qemu(1);
}
```

**重要说明**：MVP 中 panic handler 仍是 diverging 的（`-> !`），无法从 panic 中恢复。如果某个测试 panic，整个测试进程会终止。未来可通过 `setjmp`/`longjmp` 实现 panic 恢复，但 MVP 先简单处理。

- [ ] **Step 4: 更新 workspace `Cargo.toml`**

在 `Cargo.toml` 的 `[workspace] members` 中添加：

```toml
members = [
    ".",
    "crates/config",
    "crates/address",
    "crates/macros",
    "crates/per_cpu",
    "crates/interrupt_state",
    "crates/sync",
    "crates/global_tick",
    "crates/local_tick",
    "crates/frame_allocator",
    "crates/page_table_entry",
    "crates/page_table",
    "crates/memory",
    "xtask",
    "tests/system",
]
```

- [ ] **Step 5: 验证编译**

```bash
cargo build -p system-test --target riscv64gc-unknown-none-elf -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem
```

预期：编译成功。如果有链接错误（缺少符号），检查汇编文件是否正确编译，linker script 路径是否正确。

常见问题：
- `kernel_thread_bootstrap` 符号冲突：测试 binary 和内核 lib 都定义了此符号。解决：从 `lib.rs` 中移除 `kernel_thread_bootstrap`（它本就属于 binary 入口），确保只在各 binary 的 `main.rs` 中定义。
- `_start` 符号冲突：同理，`_start` 只在 binary 中定义。

- [ ] **Step 6: 提交**

```bash
git add tests/system/ Cargo.toml
git commit --signoff -m "feat(test): 创建统一测试内核 crate

添加 tests/system/ crate，依赖内核 lib，
复用汇编入口和 linker script，在 QEMU 中启动并运行测试。"
```

---

## Task 5: 编写第一批测试用例（memory + sync）

实现最基本的测试用例，验证框架端到端工作。

**Files:**
- Create: `tests/system/src/memory.rs`
- Create: `tests/system/src/sync_tests.rs`

- [ ] **Step 1: 编写 `tests/system/src/memory.rs`**

```rust
//! 内存子系统测试组

use crate::framework::TestCase;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

pub fn tests() -> &'static [TestCase] {
    &[
        TestCase {
            name: "heap_box_alloc",
            run: test_heap_box_alloc,
        },
        TestCase {
            name: "heap_vec_alloc",
            run: test_heap_vec_alloc,
        },
        TestCase {
            name: "heap_large_alloc",
            run: test_heap_large_alloc,
        },
    ]
}

/// 基本堆分配：Box
fn test_heap_box_alloc() {
    let val = Box::new(42u64);
    assert_eq!(*val, 42);
    let val2 = Box::new([0u8; 256]);
    assert_eq!(val2[0], 0);
    assert_eq!(val2[255], 0);
}

/// Vec 分配和增长
fn test_heap_vec_alloc() {
    let mut v: Vec<u32> = Vec::new();
    for i in 0..100 {
        v.push(i);
    }
    assert_eq!(v.len(), 100);
    assert_eq!(v[99], 99);
}

/// 较大块分配
fn test_heap_large_alloc() {
    let v = vec![0xAAu8; 4096];
    assert_eq!(v.len(), 4096);
    assert_eq!(v[0], 0xAA);
    assert_eq!(v[4095], 0xAA);
}
```

- [ ] **Step 2: 编写 `tests/system/src/sync_tests.rs`**

```rust
//! 同步原语测试组

use crate::framework::TestCase;
use sync::SpinLock;

pub fn tests() -> &'static [TestCase] {
    &[
        TestCase {
            name: "spinlock_basic",
            run: test_spinlock_basic,
        },
        TestCase {
            name: "spinlock_modify",
            run: test_spinlock_modify,
        },
        TestCase {
            name: "spinlock_not_held_after_drop",
            run: test_spinlock_not_held_after_drop,
        },
    ]
}

/// SpinLock 基本获取和释放
fn test_spinlock_basic() {
    let lock = SpinLock::new(42u32, "test_basic");
    let guard = lock.lock();
    assert_eq!(*guard, 42);
}

/// SpinLock 修改受保护数据
fn test_spinlock_modify() {
    let lock = SpinLock::new(0u32, "test_modify");
    {
        let mut guard = lock.lock();
        *guard = 99;
    }
    {
        let guard = lock.lock();
        assert_eq!(*guard, 99);
    }
}

/// 确认 guard drop 后锁已释放
fn test_spinlock_not_held_after_drop() {
    let lock = SpinLock::new(0u32, "test_drop");
    {
        let _guard = lock.lock();
    }
    assert!(!lock.is_locked());
}
```

- [ ] **Step 3: 提交**

```bash
git add tests/system/src/memory.rs tests/system/src/sync_tests.rs
git commit --signoff -m "feat(test): 添加 memory 和 sync 测试用例

首批系统测试：堆分配（Box/Vec/大块）和 SpinLock（获取/修改/释放）。"
```

---

## Task 6: xtask 新增 `test` 子命令

在 `xtask` 中添加 `test` 命令，编译测试内核、启动 QEMU、检查退出码。

**Files:**
- Create: `xtask/src/test.rs`
- Modify: `xtask/src/main.rs`
- Modify: `xtask/src/build.rs` (新增 `build_test_kernel`)

- [ ] **Step 1: 在 `xtask/src/build.rs` 添加 `build_test_kernel()`**

在文件末尾添加：

```rust
/// 编译系统测试内核 ELF，返回产物路径。
pub fn build_test_kernel(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    package: &str,
    release: bool,
) -> Result<PathBuf> {
    println!("[xtask] Building test kernel '{}' for {}...", package, arch.as_str());
    let target = arch.target_triple();
    let mut build_cmd = cmd!(
        sh,
        "cargo build -p {package} -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem --target {target}"
    );
    if release {
        build_cmd = build_cmd.arg("--release");
    }
    build_cmd.run()?;

    let profile_dir = if release { "release" } else { "debug" };
    let elf_path = project_root
        .join("target")
        .join(arch.target_triple())
        .join(profile_dir)
        .join(package);
    if !elf_path.exists() {
        return Err(format!("test kernel ELF not found at {}", elf_path.display()).into());
    }

    Ok(elf_path)
}
```

- [ ] **Step 2: 创建 `xtask/src/test.rs`**

```rust
//! `cargo xtask test` — 在 QEMU 中运行系统测试

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use xshell::Shell;

use crate::arch::Arch;
use crate::{Result, build, firmware, qemu};

/// 运行系统测试
pub fn run_system_test(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    release: bool,
    timeout_secs: u64,
) -> Result<bool> {
    run_test_binary(sh, project_root, arch, "system-test", release, timeout_secs)
}

/// 运行指定的独立测试
pub fn run_standalone_test(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    name: &str,
    release: bool,
    timeout_secs: u64,
) -> Result<bool> {
    run_test_binary(sh, project_root, arch, name, release, timeout_secs)
}

/// 编译并运行指定测试 binary
fn run_test_binary(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    package: &str,
    release: bool,
    timeout_secs: u64,
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

    // 启动 QEMU 并捕获输出，设置超时
    let qemu_binary = arch.qemu_binary();
    let rootfs_drive = format!("file={},if=none,format=raw,id=hd0", rootfs_path.display());
    let qemu_log = boot_dir.join("qemu.log");
    let fw = arch.firmware_dir(project_root);

    // 构建 QEMU 命令（复用 qemu.rs 的逻辑，但需要捕获输出）
    // 由于当前 launch_qemu 直接继承 stdio，这里需要用 std::process::Command
    // 手动构建以捕获 stdout/stderr
    let result = qemu::launch_qemu(sh, arch, project_root, &boot_dir, &kernel_elf_path, &rootfs_path, false);

    match result {
        Ok(()) => {
            println!("[xtask] Test '{}' completed", package);
            // QEMU 正常退出——检查退出码
            // 注意：qemu-exit crate 会让 QEMU 以特定退出码退出
            // 退出码 0 = 成功，非 0 = 失败
            Ok(true)
        }
        Err(e) => {
            eprintln!("[xtask] Test '{}' failed: {}", package, e);
            Ok(false)
        }
    }
}

/// 列出所有可用的测试
pub fn list_tests(project_root: &Path) {
    println!("Available tests:");
    println!("  system-test      — Unified system test kernel (all test groups)");

    // 扫描 standalone 测试目录
    let standalone_dir = project_root.join("tests/standalone");
    if standalone_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&standalone_dir) {
            for entry in entries.flatten() {
                if entry.path().join("Cargo.toml").exists() {
                    println!(
                        "  {}      — Standalone test",
                        entry.file_name().to_string_lossy()
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 3: 修改 `xtask/src/main.rs`，添加 Test 子命令**

在 `Commands` enum 中添加：

```rust
#[derive(Args, Clone)]
struct TestArgs {
    #[arg(long, value_enum, default_value = "riscv64")]
    arch: Arch,
    #[arg(long)]
    release: bool,
    /// 运行指定的独立测试（如 panic_test）
    #[arg(long)]
    name: Option<String>,
    /// 运行全部测试（统一 + 所有独立）
    #[arg(long)]
    all: bool,
    /// 列出可用测试
    #[arg(long)]
    list: bool,
    /// 超时秒数（默认 300）
    #[arg(long, default_value = "300")]
    timeout: u64,
}

#[derive(Subcommand)]
enum Commands {
    Build(ArchArgs),
    Run(ArchArgs),
    Debug(ArchArgs),
    Firmware(ArchArgs),
    Test(TestArgs),
}
```

在 `run()` 函数的 `match` 中添加：

```rust
Commands::Test(args) => {
    if args.list {
        test::list_tests(&project_root);
        return Ok(());
    }

    let mut all_passed = true;

    if let Some(name) = &args.name {
        // 运行指定独立测试
        let passed = test::run_standalone_test(
            &sh, &project_root, args.arch, name, args.release, args.timeout,
        )?;
        all_passed &= passed;
    } else {
        // 运行统一测试内核
        let passed = test::run_system_test(
            &sh, &project_root, args.arch, args.release, args.timeout,
        )?;
        all_passed &= passed;
    }

    if args.all {
        // 还要运行所有独立测试
        let standalone_dir = project_root.join("tests/standalone");
        if standalone_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&standalone_dir) {
                for entry in entries.flatten() {
                    if entry.path().join("Cargo.toml").exists() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let passed = test::run_standalone_test(
                            &sh, &project_root, args.arch, &name,
                            args.release, args.timeout,
                        )?;
                        all_passed &= passed;
                    }
                }
            }
        }
    }

    if !all_passed {
        eprintln!("[xtask] Some tests failed");
        process::exit(1);
    }
    println!("[xtask] All tests passed");
}
```

在文件顶部添加 `mod test;`。

- [ ] **Step 4: 验证编译**

```bash
cargo build -p xtask
```

预期：xtask 编译成功。

- [ ] **Step 5: 提交**

```bash
git add xtask/src/test.rs xtask/src/main.rs xtask/src/build.rs
git commit --signoff -m "feat(xtask): 添加 test 子命令

支持 cargo xtask test --arch riscv64 运行统一测试内核，
--name 指定独立测试，--all 运行全部，--list 列出可用测试。"
```

---

## Task 7: 端到端验证

在 QEMU 中实际运行测试，验证整个链路。

**Files:**
- 无新文件

- [ ] **Step 1: 编译并运行系统测试（RISC-V64）**

```bash
cargo xtask test --arch riscv64
```

预期输出（在 QEMU 串口中）：
```
=== SimpleKernel System Tests ===

running 3 tests in group "memory"
test heap_box_alloc ... ok
test heap_vec_alloc ... ok
test heap_large_alloc ... ok
group result: ok. 3 passed; 0 failed

running 3 tests in group "sync"
test spinlock_basic ... ok
test spinlock_modify ... ok
test spinlock_not_held_after_drop ... ok
group result: ok. 3 passed; 0 failed

================================
test result: ok. 6 passed; 0 failed
```

然后 QEMU 以退出码 0 退出。

如果失败，按以下顺序排查：
1. 编译错误 → 检查 `build.rs` 中的汇编和 linker script 路径
2. QEMU 启动但无输出 → 检查 `_start` 是否被正确链接（`llvm-nm` 查看符号）
3. panic → 检查 `kernel_init()` 的初始化顺序
4. QEMU 不退出 → 检查 `exit_qemu()` 的平台实现

- [ ] **Step 2: 编译并运行系统测试（AArch64）**

```bash
cargo xtask test --arch aarch64
```

预期：同上，测试全部通过，QEMU 正常退出。

- [ ] **Step 3: 验证内核本身仍可正常运行**

```bash
cargo xtask run --arch riscv64
```

预期：内核正常启动，烟雾测试通过（行为与此次改动前一致）。

- [ ] **Step 4: 提交（如有修复）**

如果端到端验证过程中做了修复，提交修复：

```bash
git add -u
git commit --signoff -m "fix(test): 修复端到端验证中发现的问题"
```

---

## Task 8: 创建独立 panic 测试

添加第一个独立二进制测试，验证 panic handler 行为。

**Files:**
- Create: `tests/standalone/panic_test/Cargo.toml`
- Create: `tests/standalone/panic_test/build.rs`
- Create: `tests/standalone/panic_test/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: 创建目录**

```bash
mkdir -p tests/standalone/panic_test/src
```

- [ ] **Step 2: 创建 `tests/standalone/panic_test/Cargo.toml`**

```toml
[package]
name = "panic-test"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
build = "build.rs"

[dependencies]
simplekernel = { path = "../../.." }
per_cpu = { path = "../../../crates/per_cpu" }
memory = { path = "../../../crates/memory" }
log.workspace = true
qemu-exit.workspace = true

[target.'cfg(target_arch = "riscv64")'.dependencies]
sbi-rt.workspace = true

[target.'cfg(target_arch = "aarch64")'.dependencies]
aarch64-cpu.workspace = true

[build-dependencies]
cc.workspace = true
```

- [ ] **Step 3: 创建 `tests/standalone/panic_test/build.rs`**

与 `tests/system/build.rs` 相同，但路径调整为 `../../../src/arch`：

```rust
use std::path::PathBuf;

fn compiler_for(arch: &str) -> &'static str {
    match arch {
        "riscv64" => "riscv64-linux-gnu-gcc",
        "aarch64" => "aarch64-linux-gnu-gcc",
        _ => unreachable!("unsupported architecture: {arch}"),
    }
}

fn extra_flags_for(arch: &str) -> &'static [&'static str] {
    match arch {
        "riscv64" => &["-march=rv64gc", "-mabi=lp64d"],
        _ => &[],
    }
}

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if !matches!(arch.as_str(), "riscv64" | "aarch64") || os != "none" {
        return;
    }

    let kernel_arch_dir = PathBuf::from("../../../src/arch").join(&arch);

    let mut build = cc::Build::new();
    build.compiler(compiler_for(&arch));
    for flag in extra_flags_for(&arch) {
        build.flag(flag);
    }
    build.include(&kernel_arch_dir);

    let mut has_asm = false;
    for entry in std::fs::read_dir(&kernel_arch_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", kernel_arch_dir.display()))
    {
        let path = entry
            .unwrap_or_else(|e| panic!("cannot read entry in {}: {e}", kernel_arch_dir.display()))
            .path();
        if path.extension().is_some_and(|ext| ext == "S") {
            build.file(&path);
            has_asm = true;
        }
    }
    if has_asm {
        build.compile("asm");
    }

    println!("cargo:rustc-link-arg=-z");
    println!("cargo:rustc-link-arg=norelro");
    println!(
        "cargo:rustc-link-arg=-T{}",
        kernel_arch_dir.join("link.ld").display()
    );
    println!("cargo:rerun-if-changed={}", kernel_arch_dir.display());
}
```

- [ ] **Step 4: 创建 `tests/standalone/panic_test/src/main.rs`**

```rust
//! 独立测试：验证 panic handler 正确触发
//!
//! 期望行为：触发 panic，输出包含 "PANIC_TEST_TRIGGERED"，然后退出。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

static PRIMARY_BOOTED: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
    if !PRIMARY_BOOTED.swap(true, Ordering::AcqRel) {
        panic_test_main(argc, argv);
    } else {
        // 从核：简单 idle
        loop { core::hint::spin_loop(); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(_entry: usize, _arg: usize) -> ! {
    loop { core::hint::spin_loop(); }
}

fn panic_test_main(argc: i32, argv: *const *const u8) -> ! {
    // 初始化到 Memory 级别即可
    // SAFETY: 主核首次调用
    unsafe {
        simplekernel::boot::kernel_init(
            argc,
            argv,
            simplekernel::boot::InitLevel::Memory,
        );
    }

    log::info!("PANIC_TEST: about to trigger intentional panic...");

    // 触发 panic
    panic!("PANIC_TEST_TRIGGERED: this panic is intentional");
}

// ── lang items ──

use core::alloc::Layout;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    // 打印 panic 信息
    simplekernel::logging::raw_put("PANIC_TEST_TRIGGERED\n");

    // 退出 QEMU（成功——因为触发 panic 就是我们的期望）
    #[cfg(target_arch = "riscv64")]
    {
        sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    }
    #[cfg(target_arch = "aarch64")]
    {
        use qemu_exit::QEMUExit;
        let qemu_exit = qemu_exit::AArch64::new();
        qemu_exit.exit(0);
    }
    loop { core::hint::spin_loop(); }
}

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    loop { core::hint::spin_loop(); }
}
```

- [ ] **Step 5: 更新 workspace `Cargo.toml`**

在 `members` 中添加：

```toml
"tests/standalone/panic_test",
```

- [ ] **Step 6: 验证编译和运行**

```bash
cargo build -p panic-test --target riscv64gc-unknown-none-elf -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem
cargo xtask test --arch riscv64 --name panic-test
```

预期：输出包含 `PANIC_TEST_TRIGGERED`，QEMU 正常退出。

- [ ] **Step 7: 提交**

```bash
git add tests/standalone/panic_test/ Cargo.toml
git commit --signoff -m "feat(test): 添加 panic 行为独立测试

验证 panic handler 正确触发、输出信息并安全退出 QEMU。"
```

---

## Task 9: 更新 CI 配置

将 CI 的测试步骤从 `cargo xtask run` + grep 切换到 `cargo xtask test`。

**Files:**
- Modify: `.github/workflows/workflow.yml`

- [ ] **Step 1: 更新 riscv64 测试步骤**

将 `.github/workflows/workflow.yml` 的 riscv64 System Test 步骤（约行 98-109）替换为：

```yaml
      - name: System Test
        run: |
          for i in $(seq 1 $SYSTEM_TEST_RUNS); do
            echo "=== riscv64 System Test Run $i/$SYSTEM_TEST_RUNS ==="
            timeout 300 cargo xtask test --arch riscv64 --all
            echo "riscv64 system test run $i/$SYSTEM_TEST_RUNS passed"
          done
```

- [ ] **Step 2: 更新 aarch64 测试步骤**

同理，将 aarch64 的 System Test 步骤（约行 147-158）替换为：

```yaml
      - name: System Test
        run: |
          for i in $(seq 1 $SYSTEM_TEST_RUNS); do
            echo "=== aarch64 System Test Run $i/$SYSTEM_TEST_RUNS ==="
            timeout 300 cargo xtask test --arch aarch64 --all
            echo "aarch64 system test run $i/$SYSTEM_TEST_RUNS passed"
          done
```

- [ ] **Step 3: 提交**

```bash
git add .github/workflows/workflow.yml
git commit --signoff -m "ci: 切换系统测试到 cargo xtask test

替换 grep 字符串匹配逻辑，直接依赖 xtask test 的退出码。"
```

---

## Task 10: 清理旧烟雾测试

迁移完成后，移除内核中内联的烟雾测试。

**注意**：此任务应在所有烟雾测试用例迁移到 `tests/system/` 后执行。Task 5 只迁移了 memory 和 sync 的基础用例，完整迁移需要后续 task/smp/device 等测试组就绪后才能进行。

**Files:**
- Modify: `src/main.rs` (移除 smoke_test 调用)
- Delete: `src/smoke_test.rs`

- [ ] **Step 1: 确认所有烟雾测试已迁移**

检查 `src/smoke_test.rs` 中的每个测试是否都有对应的系统测试：

| smoke_test | 对应系统测试 |
|-----------|-------------|
| SpinLock (phase2) | `sync_tests::test_spinlock_*` ✓ |
| ELF parser (phase2) | 需新增 |
| Heap/Box (phase3) | `memory::test_heap_box_alloc` ✓ |
| SMP (phase4) | 需新增 smp 测试组 |
| Lock contention (P5a) | 需新增 sync 并发测试组 |
| Sleep/clone/wait/signal (P5b) | 需新增 task 测试组 |

**结论**：Task 5 只覆盖了部分。完整迁移需要先补齐 smp、task、并发测试。此任务暂时标记为"待后续补齐后执行"。

- [ ] **Step 2: 移除 smoke_test（当所有用例已迁移时）**

从 `src/main.rs` 的 `bootstrap()` 中删除：
```rust
// 删除这些行：
mod smoke_test;
smoke_test::phase2();
smoke_test::phase3();
smoke_test::phase4();
smoke_test::spawn_all();
```

删除 `src/smoke_test.rs` 文件。

- [ ] **Step 3: 验证内核仍可正常启动**

```bash
cargo xtask run --arch riscv64
```

预期：内核正常启动，直接进入 idle loop（无烟雾测试输出）。

- [ ] **Step 4: 提交**

```bash
git rm src/smoke_test.rs
git add src/main.rs
git commit --signoff -m "refactor: 移除内联烟雾测试

所有冒烟测试已迁移到 tests/system/ 系统测试框架。"
```

---

## 实现顺序总结

```
Task 1: lib+bin 拆分          ← 基础，无依赖
Task 2: kernel_init() 接口    ← 依赖 Task 1
Task 3: 测试框架              ← 可与 Task 2 并行
Task 4: 统一测试内核 crate    ← 依赖 Task 1, 2, 3
Task 5: 首批测试用例          ← 依赖 Task 4
Task 6: xtask test 命令       ← 可与 Task 3-5 并行
Task 7: 端到端验证            ← 依赖 Task 5, 6
Task 8: panic 独立测试        ← 依赖 Task 2, 6
Task 9: CI 更新               ← 依赖 Task 7
Task 10: 清理旧烟雾测试       ← 最后执行，依赖所有测试组补齐
```

可并行的任务组：
- **组 A**: Task 1 → Task 2
- **组 B**: Task 3 (与组 A 并行)
- **组 C**: Task 6 (与 Task 3-5 并行)
