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
mod boot_info;
mod compat;
mod config;
mod elf;
mod error;
#[cfg(not(test))]
mod fdt;
#[cfg(not(test))]
mod init;
#[cfg(not(test))]
mod lang_items;
mod logging;
mod memory;
mod panic;
mod per_cpu;
#[cfg(not(test))]
mod smoke_test;
mod sync;
mod syscall;
mod task;
mod util;

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

/// 内核线程引导函数（供 switch.S 中 `kernel_thread_entry` 调用）
///
/// 新任务首次被 `switch_to` 调度运行时，从此函数开始执行。
/// 放在 kernel crate 中打破 arch→task 循环依赖。
#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(entry: usize, arg: usize) -> ! {
    // 释放 schedule() 中通过 lock_raw 获取的 sched_lock
    // SAFETY: schedule() 在 switch_to 前获取了 sched_lock，新任务首次运行时负责释放
    unsafe { task::release_sched_lock() };

    // SAFETY: entry 是由 new_kernel_thread 编码的合法 fn(usize) 指针
    let entry_fn: fn(usize) = unsafe { core::mem::transmute(entry) };
    entry_fn(arg);

    task::exit(0);
}

/// 主核引导序列
///
/// logging → DTB → FDT/BASIC_INFO → Phase2 → Memory → Phase3
/// → Interrupt → Timer → Task → SMP → Phase4 → Phase5 → Idle loop
#[cfg(not(test))]
fn bootstrap(argc: i32, argv: *const *const u8) -> ! {
    logging::init();
    init::early_init(Arch::dtb_addr(argc, argv));
    smoke_test::phase2();
    // memory::init() 返回页表，不再内部调用 arch（打破 memory↔arch 循环）
    let mut pt = memory::init();
    Arch::map_early_mmio(&mut pt).expect("failed to map early MMIO");
    // SAFETY: 页表覆盖所有内核代码/数据及早期 MMIO
    unsafe { Arch::activate_page_table(&pt) };
    log::info!("MemoryInit: paging enabled");
    memory::store_kernel_page_table(pt);
    smoke_test::phase3();
    // 必须先初始化 timer（设置 HW_FREQ 和首次超时），再开启中断。
    // 否则开启中断后挂起的 timer 中断立刻触发，handle_timer() 中
    // get_interval() 返回 0（HW_FREQ 未初始化），导致 timer 以最高
    // 频率无限触发，形成中断风暴，主线程代码永远得不到执行。
    Arch::init_timer();
    Arch::init_interrupt();

    // P5: 任务初始化（必须在 wake_secondary_cores 之前）
    task::init();
    // 注册 timer tick 回调——打破 arch→task 循环依赖
    // SAFETY: task::timer_tick 在 task::init() 后有效且生命周期为整个内核
    unsafe { arch_traits::register_timer_tick(task::timer_tick) };

    Arch::wake_secondary_cores();
    smoke_test::phase4();

    // P5: 冒烟测试——多核锁竞争 + sleep/clone/wait/signal
    smoke_test::spawn_all();

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
    memory::init_smp(|pt| {
        // SAFETY: 主核已验证页表正确性
        unsafe { Arch::activate_page_table(pt) };
    });
    task::init_smp();
    // 与主核一致：先 timer 再 interrupt，避免中断风暴
    Arch::init_timer_smp(core_id);
    Arch::init_interrupt_smp();
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
