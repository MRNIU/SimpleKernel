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

/// 主核引导序列
///
/// logging → DTB → FDT/BASIC_INFO → Phase2 → Memory → Phase3
/// → Interrupt → Timer → Task → SMP → Phase4 → Phase5 → Idle loop
#[cfg(not(test))]
fn bootstrap(argc: i32, argv: *const *const u8) -> ! {
    logging::init();
    init::early_init(Arch::dtb_addr(argc, argv));
    smoke_test::phase2();
    memory::init();
    smoke_test::phase3();
    Arch::init_interrupt();
    Arch::init_timer();

    // P5: 任务初始化（必须在 wake_secondary_cores 之前）
    task::init();

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
