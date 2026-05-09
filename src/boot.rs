// Copyright The SimpleKernel Contributors

//! 内核分级初始化接口。
//!
//! 提供 `kernel_init()` 函数，将启动序列分解为独立级别，
//! 供内核主入口和系统测试共用。

use core::cell::SyncUnsafeCell;

use crate::arch::{Arch, ArchOps};

/// 启动栈存储——`[u8; N]` 自身对齐 1 字节，但栈指针必须满足 ABI 要求
/// （RISC-V Psabi §2.1、AAPCS64 §6.2.3 均要求 sp 16 字节对齐）。
/// 用 `#[repr(C, align(16))]` newtype 显式指定 16 字节对齐，避免链接器
/// 把 BOOT_STACK 放到奇地址导致 `_boot` 设置的 sp 非法。
#[repr(C, align(16))]
struct BootStackStorage([u8; config::KERNEL_STACK_SIZE * config::MAX_CORE_COUNT]);

/// 启动栈——仅在 `_boot` 入口到 `task::init()` 首次上下文切换期间使用。
///
/// 每核 `KERNEL_STACK_SIZE` 字节，共 `MAX_CORE_COUNT` 核。
/// 声明为普通 BSS 静态变量（data 段，RW），不放入 `.bss.boot`——
/// 后者在链接脚本中位于 `__etext` 之前，会被 W^X 覆盖为 RX 导致栈不可写。
///
/// 汇编通过 `la sp, BOOT_STACK`（riscv64）/ `adrp + add` (aarch64) 引用此符号。
#[unsafe(no_mangle)]
static BOOT_STACK: SyncUnsafeCell<BootStackStorage> = SyncUnsafeCell::new(BootStackStorage(
    [0; config::KERNEL_STACK_SIZE * config::MAX_CORE_COUNT],
));

/// 内核初始化级别
pub enum InitLevel {
    /// 日志 + per_cpu + early_init + 内存子系统 + 页表激活
    Memory,
    /// Memory + 任务调度基础设施 + 定时器 + 中断控制器
    Interrupt,
    /// Interrupt + 设备/文件系统 + SMP 唤醒（完整初始化）
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
    // SAFETY: kernel_init 只从裸机 `_start` 调用，argc/argv 保留架构启动入口传入的原始参数。
    let dtb_addr = unsafe { Arch::dtb_addr(argc, argv) };
    crate::init::early_init(dtb_addr);

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
    Arch::map_early_mmio();
    // SAFETY: 页表覆盖所有内核代码/数据及早期 MMIO
    {
        let pt = paging::kernel_page_table();
        unsafe { Arch::activate_page_table(pt) };
    }
    log::info!("MemoryInit: paging enabled");

    // 冒烟测试：堆分配 + 帧分配
    smoke_test_memory();

    if matches!(level, InitLevel::Memory) {
        return;
    }

    crate::task::init();

    // 必须先初始化 timer（设置 HW_FREQ 和首次超时），再开启中断。
    // task::init() 必须在开启中断前完成，timer IRQ exit 可能触发抢占调度。
    Arch::init_timer();
    Arch::init_interrupt();
    crate::tlb_shootdown::init_primary();

    if matches!(level, InitLevel::Interrupt) {
        return;
    }

    // P6/P7: 设备 + 文件系统初始化
    // 必须在 task::init() 之后——timer 中断可能触发 schedule()，
    // 需要 per-CPU 调度器已初始化。
    // 必须在 wake_secondary_cores() 之前——避免从核竞争。
    crate::device::device_init();
    crate::fs::fs_init();

    Arch::wake_secondary_cores();
    crate::tlb_shootdown::wait_for_all_discovered_cores_online();
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
        frame_allocator::AllocatedFrames::alloc_one().expect("boot smoke: frame alloc_one failed");
    assert!(
        frame
            .start_paddr()
            .as_usize()
            .is_multiple_of(config::PAGE_SIZE),
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
    crate::tlb_shootdown::mark_current_core_online();
    log::info!("SMP: core {} online", core_id);
}
