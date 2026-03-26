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
#[cfg(not(test))]
mod smoke_test;
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
