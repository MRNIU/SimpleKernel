#![no_std]
#![no_main]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use simplekernel::arch::{Arch, ArchOps};
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
/// logging → DTB → FDT → Phase2 → Memory → Phase3
/// → Interrupt → Timer → Task → SMP → Phase4 → Phase5 → Idle loop
fn bootstrap(argc: i32, argv: *const *const u8) -> ! {
    logging::init();
    // SAFETY: 主核调用一次，TP 持有 hart_id（riscv64）/ TPIDR_EL1 为 0（aarch64）
    unsafe { per_cpu::percpu_init() };
    init::early_init(Arch::dtb_addr(argc, argv));
    smoke_test::phase2();
    // memory::init() 返回内核地址空间（页表已存入全局）
    let mut kernel_as = memory::init();
    Arch::map_early_mmio(&mut kernel_as).expect("failed to map early MMIO");
    // SAFETY: 页表覆盖所有内核代码/数据及早期 MMIO
    {
        let pt = paging::kernel_page_table().lock();
        unsafe { Arch::activate_page_table(&pt) };
    }
    log::info!("MemoryInit: paging enabled");
    memory::store_kernel_address_space(kernel_as);
    smoke_test::phase3();
    // 必须先初始化 timer（设置 HW_FREQ 和首次超时），再开启中断。
    // 否则开启中断后挂起的 timer 中断立刻触发，handle_timer() 中
    // get_interval() 返回 0（HW_FREQ 未初始化），导致 timer 以最高
    // 频率无限触发，形成中断风暴，主线程代码永远得不到执行。
    Arch::init_timer();
    Arch::init_interrupt();

    // P6: 设备子系统——FDT 枚举 + VirtIO 探测
    simplekernel::device::device_init();

    // P7: 文件系统——挂载 RamFS + VFS 冒烟测试
    simplekernel::fs::fs_init();

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
