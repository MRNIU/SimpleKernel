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
    crate::logging::init();
    // SAFETY: 主核调用一次，TP 持有 hart_id（riscv64）/ TPIDR_EL1 为 0（aarch64）
    unsafe { per_cpu::percpu_init() };
    crate::init::early_init(Arch::dtb_addr(argc, argv));

    // ELF 符号表初始化——panic backtrace 依赖此信息
    let elf_addr = memory::MEMORY_INFO
        .get()
        .expect("MEMORY_INFO not initialized")
        .kernel_addr
        .as_usize() as u64;
    // SAFETY: elf_addr 是内核自身的 ELF 基地址，在内核生命周期内有效
    unsafe { crate::panic::init_elf(elf_addr) };

    // 冒烟测试：SpinLock 基本操作
    smoke_test_spinlock();

    memory::init();
    Arch::map_early_mmio().expect("failed to map early MMIO");
    // SAFETY: 页表覆盖所有内核代码/数据及早期 MMIO
    {
        let pt = paging::kernel_page_table().lock();
        unsafe { Arch::activate_page_table(&pt) };
    }
    log::info!("MemoryInit: paging enabled");

    // 冒烟测试：堆分配 + 帧分配
    smoke_test_memory();

    if matches!(level, InitLevel::Memory) {
        return;
    }

    // 必须先初始化 timer（设置 HW_FREQ 和首次超时），再开启中断。
    // 否则开启中断后挂起的 timer 中断立刻触发，handle_timer() 中
    // get_interval() 返回 0（HW_FREQ 未初始化），导致 timer 以最高
    // 频率无限触发，形成中断风暴，主线程代码永远得不到执行。
    Arch::init_timer();
    Arch::init_interrupt();

    if matches!(level, InitLevel::Interrupt) {
        return;
    }

    crate::task::init();

    // P6/P7: 设备 + 文件系统初始化
    // 必须在 task::init() 之后——timer 中断可能触发 schedule()，
    // 需要 per-CPU 调度器已初始化。
    // 必须在 wake_secondary_cores() 之前——避免从核竞争。
    crate::device::device_init();
    crate::fs::fs_init();

    Arch::wake_secondary_cores();
}

/// 冒烟测试：SpinLock 创建、加锁、修改、解锁。
fn smoke_test_spinlock() {
    let lock = sync::SpinLock::new(42u32, "boot_smoke", sync::lock_level::UNSPECIFIED);
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
    log::debug!("boot smoke: SpinLock OK");
}

/// 冒烟测试：堆分配（Vec）+ 帧分配（alloc_one）+ aarch64 TLBI。
fn smoke_test_memory() {
    // 堆分配验证
    let v = alloc::vec![1u32, 2, 3];
    assert_eq!(v.len(), 3);
    assert_eq!(v[0] + v[1] + v[2], 6);
    log::debug!("boot smoke: heap alloc OK");

    // 帧分配验证
    let frame =
        memory::frame::AllocatedFrames::alloc_one().expect("boot smoke: frame alloc_one failed");
    assert!(
        frame.start_paddr().as_usize() % config::PAGE_SIZE == 0,
        "boot smoke: frame not page-aligned: {:#x}",
        frame.start_paddr().as_usize()
    );
    log::debug!("boot smoke: frame alloc OK");

    // aarch64: 基本 TLBI 验证（vmalle1 全局 TLB 无效化）
    #[cfg(target_arch = "aarch64")]
    {
        use aarch64_cpu::asm::{barrier, tlbi};

        barrier::dsb(barrier::SY);
        tlbi::vmalle1();
        barrier::dsb(barrier::SY);
        barrier::isb(barrier::SY);
        log::debug!("boot smoke: TLBI vmalle1 OK");
    }
}

/// 从核初始化序列。
///
/// # Safety
/// 必须在从核的汇编入口跳转后调用，per-CPU 寄存器已设置。
pub unsafe fn kernel_init_smp() {
    // SAFETY: percpu_init() 已由主核完成，arch::core_id() 返回有效核心 ID
    unsafe { per_cpu::percpu_init_smp() };
    let core_id = per_cpu::current_core_id();
    memory::init_smp(|pt| {
        // SAFETY: 主核已验证页表正确性
        unsafe { Arch::activate_page_table(pt) };
    });
    crate::task::init_smp();
    // 与主核一致：先 timer 再 interrupt，避免中断风暴
    Arch::init_timer_smp(core_id);
    Arch::init_interrupt_smp();
    log::info!("SMP: core {} online", core_id);
}
