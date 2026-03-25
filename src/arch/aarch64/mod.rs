pub mod console;
pub mod context;
pub mod init;
pub mod interrupt;
pub mod ipi;
pub mod syscall;
pub mod timer;

/// 主核引导入口
///
/// 顺序：arch_init → phase2 → memory_init → phase3 → interrupt_init → timer_init →
///       wake_up_other_cores → phase4_smoke_test → 空转循环
pub fn bootstrap(argc: i32, argv: *const *const u8) -> ! {
    init::arch_init(argc, argv);
    crate::phase2_smoke_test();
    crate::memory::memory_init();
    crate::phase3_smoke_test();
    interrupt::interrupt_init();
    timer::timer_init();
    ipi::wake_up_other_cores();
    crate::phase4_smoke_test();
    loop {
        core::hint::spin_loop();
    }
}

/// 从核引导入口
///
/// 通过 MPIDR_EL1 获取当前 CPU ID。
pub fn bootstrap_smp(_argc: i32, _argv: *const *const u8) -> ! {
    let cpu_id = crate::per_cpu::current_core_id();
    init::arch_init_smp(cpu_id);
    crate::memory::memory_init_smp();
    interrupt::interrupt_init_smp();
    timer::timer_init_smp(cpu_id);
    log::info!("SMP: core {} online", cpu_id);
    loop {
        core::hint::spin_loop();
    }
}

/// 内核线程引导存根（供 switch.S 调用）
///
/// TODO(P5)：实现任务入口启动逻辑。
#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(_entry: usize, _arg: usize) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
