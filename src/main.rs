#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), feature(alloc_error_handler))]
#![feature(sync_unsafe_cell)]
// 测试模式下部分模块不编译（arch, fdt, lang_items），导致它们的消费者
// 产生 dead_code 警告。这些代码在目标架构上被正常使用。
#![cfg_attr(test, allow(dead_code))]

#[cfg(not(test))]
extern crate alloc;

#[cfg(not(test))]
mod arch;
mod config;
mod elf;
mod error;
#[cfg(not(test))]
mod fdt;
mod fmt_buf;
mod halt;
#[cfg(not(test))]
mod lang_items;
mod logging;
mod memory;
mod panic;
mod per_cpu;
mod scope_guard;
mod sync;
mod syscall;
mod task;

#[cfg(not(test))]
use core::sync::atomic::{AtomicBool, Ordering};

/// 标记主核是否已完成初始化，用于区分主核/从核引导路径
#[cfg(not(test))]
static PRIMARY_BOOTED: AtomicBool = AtomicBool::new(false);

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
    // swap 返回旧值：false → 当前核是第一个到达的核（主核）
    if !PRIMARY_BOOTED.swap(true, Ordering::AcqRel) {
        bootstrap(argc, argv);
    } else {
        bootstrap_smp(argc, argv);
    }
}

#[cfg(not(test))]
use arch::{Arch, ArchOps};

/// 主核引导序列
///
/// logging → DTB → FDT/BASIC_INFO → Phase2 → Memory → Phase3
/// → Interrupt → Timer → Task → SMP → Phase4 → Phase5 → Idle loop
#[cfg(not(test))]
fn bootstrap(argc: i32, argv: *const *const u8) -> ! {
    logging::init();
    early_init(Arch::dtb_addr(argc, argv));
    phase2_smoke_test();
    memory::init();
    phase3_smoke_test();
    Arch::init_interrupt();
    Arch::init_timer();

    // P5: 任务初始化（必须在 wake_secondary_cores 之前）
    task::init();

    Arch::wake_secondary_cores();
    phase4_smoke_test();

    // P5a: 多核锁竞争测试
    task::spawn_kernel_thread("counter_0", counter_thread, 0);
    task::spawn_kernel_thread("counter_1", counter_thread, 1);
    task::spawn_kernel_thread("counter_2", counter_thread, 2);
    task::spawn_kernel_thread("counter_3", counter_thread, 3);
    task::spawn_kernel_thread("verifier", verifier_thread, 0);

    // P5b: sleep + clone/wait + KMutex + signal 测试
    task::spawn_kernel_thread("p5b_test", p5b_test_thread, 0);

    phase5_smoke_test();

    // 立即尝试调度，开始运行刚创建的线程
    task::schedule();

    // Idle loop — bootstrap 上下文成为 idle 任务
    loop {
        if per_cpu::check_and_clear_need_resched() {
            task::schedule();
        }
        core::hint::spin_loop();
    }
}

/// 从核引导序列
///
/// 由主核通过 `wake_secondary_cores()` 启动，经 `_start` 分流到此。
#[cfg(not(test))]
fn bootstrap_smp(argc: i32, argv: *const *const u8) -> ! {
    let core_id = Arch::secondary_core_id(argc, argv);
    memory::init_smp();
    Arch::init_interrupt_smp();
    task::init_smp();
    Arch::init_timer_smp(core_id);
    log::info!("SMP: core {} online", core_id);

    // 从核上线后立即尝试调度，抢全局队列中的任务
    task::schedule();

    // Idle loop
    loop {
        if per_cpu::check_and_clear_need_resched() {
            task::schedule();
        }
        core::hint::spin_loop();
    }
}

/// 早期初始化（架构无关）——解析 FDT，填充 BASIC_INFO。
///
/// 在堆和分页启用之前运行，仅依赖 logging 和栈。
#[cfg(not(test))]
fn early_init(dtb_addr: usize) {
    use fdt::KernelFdt;
    use memory::address::PhysAddr;
    use per_cpu::{BASIC_INFO, BasicInfo};

    let fdt = match KernelFdt::new(dtb_addr) {
        Ok(f) => f,
        Err(_) => {
            logging::raw_put("FATAL: Failed to parse FDT\n");
            loop {
                core::hint::spin_loop();
            }
        }
    };

    let node_count = fdt.node_count().unwrap_or(0);
    let core_count = fdt.core_count().unwrap_or(1);

    let (mem_addr, mem_size) = match fdt.memory() {
        Ok(m) => m,
        Err(_) => {
            logging::raw_put("FATAL: Failed to get memory info from FDT\n");
            loop {
                core::hint::spin_loop();
            }
        }
    };

    // SAFETY: 链接器定义的符号，地址在内核生命周期内有效
    unsafe extern "C" {
        static __executable_start: u8;
        static _end: u8;
    }
    let kernel_start = unsafe { &__executable_start as *const u8 as u64 };
    let kernel_end = unsafe { &_end as *const u8 as u64 };

    BASIC_INFO.call_once(|| BasicInfo {
        physical_memory_addr: PhysAddr::new(mem_addr as usize),
        physical_memory_size: mem_size,
        kernel_addr: PhysAddr::new(kernel_start as usize),
        kernel_size: (kernel_end - kernel_start) as usize,
        elf_addr: PhysAddr::new(kernel_start as usize),
        fdt_addr: PhysAddr::new(dtb_addr),
        core_count,
    });

    log::info!("FDT: found {} nodes, {} CPUs", node_count, core_count);
    log::info!("Memory: {} MB", mem_size / (1024 * 1024));
    log::info!("Hello SimpleKernel");
}

#[cfg(not(test))]
fn phase2_smoke_test() {
    use sync::SpinLock;

    log::info!("Testing SpinLock...");
    let lock = SpinLock::new(42u32, "smoke_test");
    {
        let mut guard = lock.lock();
        assert_eq!(*guard, 42);
        *guard = 99;
    }
    {
        let guard = lock.lock();
        assert_eq!(*guard, 99);
    }
    assert!(!lock.is_locked());
    log::info!("SpinLock OK");

    log::info!("Initializing ELF parser...");
    let elf_addr = per_cpu::BASIC_INFO
        .get()
        .expect("BASIC_INFO not initialized")
        .elf_addr
        .as_usize() as u64;
    // SAFETY: elf_addr 是内核自身的 ELF 基地址，在内核生命周期内有效
    unsafe { panic::init_elf(elf_addr) };
    log::info!("ELF parser OK");

    log::info!("Phase 2 complete");
}

#[cfg(not(test))]
fn phase3_smoke_test() {
    use alloc::boxed::Box;

    let val = Box::new(42u64);
    log::info!("HeapTest: Box::new(42) = {}", *val);
    assert_eq!(*val, 42);

    log::info!("Phase 3 complete");
}

#[cfg(not(test))]
fn phase4_smoke_test() {
    log::info!("Phase 4 complete");
}

// ─── P5 多核 + 锁竞争测试 ──────────────────────────────────────────────────

#[cfg(not(test))]
use core::sync::atomic::AtomicU32;

/// 多核锁竞争测试：多个线程在不同核心上对同一 SpinLock 保护的计数器做自增。
/// 若锁实现正确，最终计数应等于 线程数 × 每线程迭代数。
#[cfg(not(test))]
static TEST_COUNTER: sync::SpinLock<u32> = sync::SpinLock::new(0, "test_counter");

/// 已完成的测试线程数
#[cfg(not(test))]
static TEST_DONE: AtomicU32 = AtomicU32::new(0);

/// 每个线程的迭代次数
#[cfg(not(test))]
const ITERS_PER_THREAD: u32 = 200;

/// 测试线程数
#[cfg(not(test))]
const TEST_THREAD_COUNT: u32 = 4;

/// 锁竞争测试线程——每个线程做 ITERS_PER_THREAD 次 lock-increment-unlock-yield 循环。
/// 线程 ID 通过 arg 传入。
#[cfg(not(test))]
fn counter_thread(id: usize) {
    let core = per_cpu::current_core_id();
    log::info!("counter_{}: start on core {}", id, core);

    for i in 0..ITERS_PER_THREAD {
        {
            let mut guard = TEST_COUNTER.lock();
            *guard += 1;
        }
        // 每 50 次报告一次当前核心，验证是否发生了跨核调度
        if i % 50 == 0 {
            let core = per_cpu::current_core_id();
            log::info!("counter_{}: i={} on core {}", id, i, core);
        }
        task::yield_now();
    }

    TEST_DONE.fetch_add(1, Ordering::Release);
    let core = per_cpu::current_core_id();
    log::info!("counter_{}: done on core {}", id, core);
}

/// 验证线程——等所有 counter 线程完成后检查计数是否正确
#[cfg(not(test))]
fn verifier_thread(_arg: usize) {
    // 轮询等待所有 counter 线程完成
    loop {
        if TEST_DONE.load(Ordering::Acquire) >= TEST_THREAD_COUNT {
            break;
        }
        task::yield_now();
    }

    let count = *TEST_COUNTER.lock();
    let expected = TEST_THREAD_COUNT * ITERS_PER_THREAD;
    if count == expected {
        log::info!(
            "=== LOCK TEST PASSED: counter={} (expected {}) ===",
            count,
            expected
        );
    } else {
        log::error!(
            "=== LOCK TEST FAILED: counter={} (expected {}) ===",
            count,
            expected
        );
    }
}

// ─── P5b 集成测试 ────────────────────────────────────────────────────────────

/// P5b 综合测试线程——验证 sleep、clone/wait、KMutex、signal
#[cfg(not(test))]
fn p5b_test_thread(_arg: usize) {
    // ── 测试 1: sleep ──
    log::info!("P5b: testing sleep...");
    let tick_before = arch::Arch::get_current_tick();
    task::sleep(2); // 睡眠 2 个 tick
    let tick_after = arch::Arch::get_current_tick();
    let elapsed = tick_after.saturating_sub(tick_before);
    log::info!("P5b: sleep(2) elapsed {} ticks", elapsed);
    if elapsed >= 2 {
        log::info!("=== SLEEP TEST PASSED ===");
    } else {
        log::error!("=== SLEEP TEST FAILED: elapsed={} < 2 ===", elapsed);
    }

    // ── 测试 2: clone + wait ──
    log::info!("P5b: testing clone/wait...");
    match task::clone_kernel_thread("child", child_thread, 42) {
        Ok(child_pid) => {
            log::info!("P5b: spawned child pid={}", child_pid);
            match task::wait_child(child_pid) {
                Ok((pid, code)) => {
                    if pid == child_pid && code == 7 {
                        log::info!("=== CLONE/WAIT TEST PASSED: pid={} code={} ===", pid, code);
                    } else {
                        log::error!(
                            "=== CLONE/WAIT TEST FAILED: pid={} code={} (expected {}:7) ===",
                            pid,
                            code,
                            child_pid
                        );
                    }
                }
                Err(e) => log::error!("=== CLONE/WAIT TEST FAILED: wait err={:?} ===", e),
            }
        }
        Err(e) => log::error!("=== CLONE/WAIT TEST FAILED: clone err={:?} ===", e),
    }

    // ── 测试 3: KMutex（阻塞互斥锁） ──
    log::info!("P5b: testing KMutex...");
    static KMUTEX: spin::Once<task::KMutex> = spin::Once::new();
    KMUTEX.call_once(task::KMutex::new);
    let km = KMUTEX.get().expect("KMutex not initialized");
    km.lock();
    log::info!("P5b: KMutex acquired");
    km.unlock();
    log::info!("=== KMUTEX TEST PASSED ===");

    // ── 测试 4: signal (SIGKILL) ──
    log::info!("P5b: testing signal...");
    match task::clone_kernel_thread("victim", victim_thread, 0) {
        Ok(victim_pid) => {
            // 让 victim 有机会运行
            task::yield_now();
            // 发送 SIGKILL
            match task::send_signal(victim_pid, task::signal::Signal::SIGKILL) {
                Ok(()) => {
                    log::info!("P5b: sent SIGKILL to pid={}", victim_pid);
                    // 等待 victim 被回收
                    match task::wait_child(victim_pid) {
                        Ok((pid, code)) => {
                            log::info!(
                                "=== SIGNAL TEST PASSED: pid={} killed, code={} ===",
                                pid,
                                code
                            );
                        }
                        Err(e) => log::error!("=== SIGNAL TEST FAILED: wait err={:?} ===", e),
                    }
                }
                Err(e) => log::error!("=== SIGNAL TEST FAILED: send err={:?} ===", e),
            }
        }
        Err(e) => log::error!("=== SIGNAL TEST FAILED: clone err={:?} ===", e),
    }

    log::info!("=== ALL P5b TESTS COMPLETE ===");
}

/// clone/wait 测试的子线程——执行一些工作后以 code=7 退出
#[cfg(not(test))]
fn child_thread(arg: usize) {
    log::info!("child_thread: arg={}", arg);
    task::yield_now();
    task::exit(7);
}

/// signal 测试的受害线程——循环 yield，等待被 SIGKILL
#[cfg(not(test))]
fn victim_thread(_arg: usize) {
    log::info!("victim_thread: running, waiting for signal...");
    loop {
        task::yield_now();
    }
}

#[cfg(not(test))]
fn phase5_smoke_test() {
    log::info!(
        "Phase 5: spawning {} counter threads + 1 verifier + P5b tests",
        TEST_THREAD_COUNT
    );
}
