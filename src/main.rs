#![no_std]
#![no_main]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use simplekernel::*;

mod smoke_test;

/// 标记主核是否已完成初始化，用于区分主核/从核引导路径
static PRIMARY_BOOTED: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
    // swap 返回旧值：false → 当前核是第一个到达的核（主核）
    if !PRIMARY_BOOTED.swap(true, Ordering::AcqRel) {
        bootstrap(argc, argv);
    } else {
        bootstrap_smp(argc, argv);
    }
}

/// 内核线程引导函数（供 switch.S 中 `kernel_thread_entry` 调用）
///
/// 新任务首次被 `switch_to` 调度运行时，从此函数开始执行。
/// 放在 kernel crate 中打破 arch→task 循环依赖。
#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(entry: usize, arg: usize) -> ! {
    // 启用中断——schedule() 的 HeldInterrupts::hold() 禁用了中断，
    // 调度锁已在 switch_to 前由 RAII guard 释放，此处只需恢复中断
    // SAFETY: 调度锁已释放，启用中断是安全的
    unsafe { task::bootstrap_enable_irq() };

    // SAFETY: entry 是由 new_kernel_thread 编码的合法 fn(usize) 指针
    let entry_fn: fn(usize) = unsafe { core::mem::transmute(entry) };
    entry_fn(arg);

    simplekernel::syscall::process::exit(0);
}

/// 主核引导序列
///
/// kernel_init(Full) 完成全部子系统初始化，之后启动冒烟测试线程并进入 idle loop。
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

/// 从核引导序列
///
/// 由主核通过 `wake_secondary_cores()` 启动，经 `_start` 分流到此。
fn bootstrap_smp(_argc: i32, _argv: *const *const u8) -> ! {
    // SAFETY: 从核入口，汇编已设置栈和寄存器
    unsafe { boot::kernel_init_smp() };

    // 从核上线后立即尝试调度，抢全局队列中的任务
    task::schedule();

    // Idle loop
    loop {
        if preempt::check_and_clear_need_resched() {
            task::schedule();
        }
        core::hint::spin_loop();
    }
}
