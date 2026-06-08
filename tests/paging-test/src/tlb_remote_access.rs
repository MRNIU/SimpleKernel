// Copyright The SimpleKernel Contributors

//! TLB shootdown 远端访问强证明。
//!
//! 测试路径：
//! 1. CPU1 先对目标 VA 写入，缓存旧 RW TLB 翻译；
//! 2. CPU0 将同一页改为 RO，并等待 TLB shootdown ack；
//! 3. CPU1 再写同一 VA，必须触发写权限异常，并由测试钩子恢复。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[cfg(any(target_arch = "riscv64", target_arch = "aarch64"))]
mod remote_access {
    use core::hint::spin_loop;
    use core::sync::atomic::{AtomicUsize, Ordering};

    use frame_allocator::AllocatedFrames;
    use paging::{PteFlags, PteFlagsOps};
    use simplekernel::{CORE_COUNT, boot::InitLevel, tlb_shootdown};

    const CMD_NONE: usize = 0;
    const CMD_WRITE: usize = 1;
    const WAIT_ROUNDS: usize = 10_000_000;
    const FIRST_VALUE: u64 = 0x1111_2222_3333_4444;
    const SECOND_VALUE: u64 = 0x5555_6666_7777_8888;

    static PRIMARY_BOOTED: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);
    static REMOTE_READY_MASK: AtomicUsize = AtomicUsize::new(0);
    static TARGET_CORE_ID: AtomicUsize = AtomicUsize::new(usize::MAX);
    static PROBE_ADDR: AtomicUsize = AtomicUsize::new(0);
    static PROBE_VALUE: AtomicUsize = AtomicUsize::new(0);
    static COMMAND: AtomicUsize = AtomicUsize::new(CMD_NONE);
    static REQUEST_SEQ: AtomicUsize = AtomicUsize::new(0);
    static ACK_SEQ: AtomicUsize = AtomicUsize::new(0);

    #[cfg(target_arch = "riscv64")]
    mod arch_probe {
        core::arch::global_asm!(
            r#"
            .section .text
            .global tlb_remote_store_probe
            .type tlb_remote_store_probe, @function
tlb_remote_store_probe:
            .global tlb_remote_store_probe_fault_pc
tlb_remote_store_probe_fault_pc:
            sd a1, 0(a0)
            fence rw, rw
            .global tlb_remote_store_probe_resume_pc
tlb_remote_store_probe_resume_pc:
            ret
            .size tlb_remote_store_probe, . - tlb_remote_store_probe
"#
        );

        unsafe extern "C" {
            fn tlb_remote_store_probe(addr: *mut u64, value: u64);
            static tlb_remote_store_probe_fault_pc: u8;
            static tlb_remote_store_probe_resume_pc: u8;
        }

        pub unsafe fn write(addr: *mut u64, value: u64) {
            // SAFETY: 调用方保证目标地址和预期 fault 恢复点已按测试阶段配置。
            unsafe { tlb_remote_store_probe(addr, value) };
        }

        pub fn expect_write_fault(target_core_id: usize, fault_addr: usize) {
            simplekernel::test_support::expect_riscv64_store_page_fault(
                target_core_id,
                fault_addr,
                fault_pc(),
                resume_pc(),
            );
        }

        pub fn write_fault_observed() -> bool {
            simplekernel::test_support::riscv64_store_page_fault_observed()
        }

        fn fault_pc() -> usize {
            core::ptr::addr_of!(tlb_remote_store_probe_fault_pc) as usize
        }

        fn resume_pc() -> usize {
            core::ptr::addr_of!(tlb_remote_store_probe_resume_pc) as usize
        }
    }

    #[cfg(target_arch = "aarch64")]
    mod arch_probe {
        core::arch::global_asm!(
            r#"
            .section .text
            .global tlb_remote_store_probe
            .type tlb_remote_store_probe, %function
tlb_remote_store_probe:
            .global tlb_remote_store_probe_fault_pc
tlb_remote_store_probe_fault_pc:
            str x1, [x0]
            dsb sy
            .global tlb_remote_store_probe_resume_pc
tlb_remote_store_probe_resume_pc:
            ret
            .size tlb_remote_store_probe, . - tlb_remote_store_probe
"#
        );

        unsafe extern "C" {
            fn tlb_remote_store_probe(addr: *mut u64, value: u64);
            static tlb_remote_store_probe_fault_pc: u8;
            static tlb_remote_store_probe_resume_pc: u8;
        }

        pub unsafe fn write(addr: *mut u64, value: u64) {
            // SAFETY: 调用方保证目标地址和预期 fault 恢复点已按测试阶段配置。
            unsafe { tlb_remote_store_probe(addr, value) };
        }

        pub fn expect_write_fault(target_core_id: usize, fault_addr: usize) {
            simplekernel::test_support::expect_aarch64_write_data_abort(
                target_core_id,
                fault_addr,
                fault_pc(),
                resume_pc(),
            );
        }

        pub fn write_fault_observed() -> bool {
            simplekernel::test_support::aarch64_write_data_abort_observed()
        }

        fn fault_pc() -> usize {
            core::ptr::addr_of!(tlb_remote_store_probe_fault_pc) as usize
        }

        fn resume_pc() -> usize {
            core::ptr::addr_of!(tlb_remote_store_probe_resume_pc) as usize
        }
    }

    pub extern "C" fn start(argc: i32, argv: *const *const u8) -> ! {
        if !PRIMARY_BOOTED.swap(true, Ordering::AcqRel) {
            // SAFETY: bare-metal 环境，主核首次调用。
            unsafe {
                simplekernel::boot::kernel_init(argc, argv, InitLevel::Full);
            }
            run_tests();
            log::info!("TEST OK");
            test_harness::exit_qemu(0);
        } else {
            // SAFETY: 从核入口，汇编已设置栈和寄存器。
            unsafe { simplekernel::boot::kernel_init_smp() };
            secondary_probe_loop();
        }
    }

    fn run_tests() {
        let target_core_id = select_remote_core();
        wait_remote_loop_ready(target_core_id);
        test_remote_core_observes_ro_after_ack(target_core_id);
        log::info!("test remote_core_observes_ro_after_ack ... ok");
        log::info!("tlb-remote-access-test: all 1 tests passed");
    }

    fn select_remote_core() -> usize {
        let expected = *CORE_COUNT
            .get()
            .expect("tlb remote access 测试读取 CORE_COUNT 失败");
        assert!(
            expected >= 2,
            "tlb remote access 测试需要至少 2 个 CPU: expected={expected}"
        );

        for _ in 0..WAIT_ROUNDS {
            if tlb_shootdown::online_core_count() == expected {
                break;
            }
            spin_loop();
        }
        assert_eq!(
            tlb_shootdown::online_core_count(),
            expected,
            "tlb remote access 测试等待所有 CPU online 超时: expected={}, actual={}",
            expected,
            tlb_shootdown::online_core_count()
        );

        let self_bit = 1usize << per_cpu::current_core_id();
        let remote_mask = tlb_shootdown::online_core_mask() & !self_bit;
        assert_ne!(
            remote_mask,
            0,
            "tlb remote access 测试未找到远端 CPU: online_mask={:#x}, self_core={}",
            tlb_shootdown::online_core_mask(),
            per_cpu::current_core_id()
        );
        remote_mask.trailing_zeros() as usize
    }

    fn wait_remote_loop_ready(target_core_id: usize) {
        let target_bit = 1usize << target_core_id;
        for _ in 0..WAIT_ROUNDS {
            if REMOTE_READY_MASK.load(Ordering::Acquire) & target_bit != 0 {
                return;
            }
            spin_loop();
        }
        panic!(
            "tlb remote access 测试等待远端 loop ready 超时: target_core={}, ready_mask={:#x}",
            target_core_id,
            REMOTE_READY_MASK.load(Ordering::Acquire)
        );
    }

    fn test_remote_core_observes_ro_after_ack(target_core_id: usize) {
        let frames = AllocatedFrames::alloc_one().expect("tlb remote access 测试分配物理帧失败");
        let vaddr = frames.start_paddr().to_virt();
        let addr = vaddr.as_usize();
        let ptr = addr as *mut u64;
        let page_table = paging::kernel_page_table();

        issue_remote_write(target_core_id, addr, FIRST_VALUE);
        assert_eq!(
            read_probe_value(ptr),
            FIRST_VALUE,
            "tlb remote access 测试第一次远端写入未生效: addr={addr:#x}"
        );

        arch_probe::expect_write_fault(target_core_id, addr);
        page_table.update_range_flags(vaddr, 1, PteFlags::kernel_ro());

        issue_remote_write(target_core_id, addr, SECOND_VALUE);
        assert!(
            arch_probe::write_fault_observed(),
            "tlb remote access 测试未观察到远端写权限异常: target_core={}, addr={addr:#x}",
            target_core_id
        );
        assert_eq!(
            read_probe_value(ptr),
            FIRST_VALUE,
            "tlb remote access 测试远端写入穿透 RO 权限: target_core={}, addr={addr:#x}",
            target_core_id
        );

        page_table.update_range_flags(vaddr, 1, PteFlags::kernel_rw());
    }

    fn issue_remote_write(target_core_id: usize, addr: usize, value: u64) {
        let next_seq = REQUEST_SEQ.load(Ordering::Relaxed).wrapping_add(1);
        assert_ne!(next_seq, 0, "tlb remote access 测试 request seq 回绕为 0");

        TARGET_CORE_ID.store(target_core_id, Ordering::Relaxed);
        PROBE_ADDR.store(addr, Ordering::Relaxed);
        PROBE_VALUE.store(value as usize, Ordering::Relaxed);
        COMMAND.store(CMD_WRITE, Ordering::Relaxed);
        REQUEST_SEQ.store(next_seq, Ordering::Release);

        for _ in 0..WAIT_ROUNDS {
            if ACK_SEQ.load(Ordering::Acquire) == next_seq {
                return;
            }
            spin_loop();
        }

        panic!(
            "tlb remote access 测试等待远端写入 ack 超时: target_core={}, addr={addr:#x}, seq={}, ack={}",
            target_core_id,
            next_seq,
            ACK_SEQ.load(Ordering::Acquire)
        );
    }

    fn secondary_probe_loop() -> ! {
        let core_id = per_cpu::current_core_id();
        REMOTE_READY_MASK.fetch_or(1usize << core_id, Ordering::Release);
        let mut seen_seq = REQUEST_SEQ.load(Ordering::Acquire);

        loop {
            let seq = REQUEST_SEQ.load(Ordering::Acquire);
            if seq == seen_seq {
                spin_loop();
                continue;
            }
            seen_seq = seq;

            if TARGET_CORE_ID.load(Ordering::Relaxed) != core_id {
                continue;
            }

            match COMMAND.load(Ordering::Relaxed) {
                CMD_WRITE => {
                    let addr = PROBE_ADDR.load(Ordering::Relaxed) as *mut u64;
                    let value = PROBE_VALUE.load(Ordering::Relaxed) as u64;
                    // SAFETY: 主核只发布由 AllocatedFrames 持有且页对齐的目标地址；
                    // 第一次写入时 PTE 为 RW，第二次写入时测试已注册精确的预期
                    // 写权限异常恢复点，trap handler 会跳到 resume label。
                    unsafe { arch_probe::write(addr, value) };
                    ACK_SEQ.store(seq, Ordering::Release);
                }
                command => {
                    panic!(
                        "tlb remote access 测试收到未知远端命令: core={}, command={command}",
                        core_id
                    );
                }
            }
        }
    }

    fn read_probe_value(ptr: *const u64) -> u64 {
        // SAFETY: ptr 指向当前测试持有的 AllocatedFrames identity mapping。
        unsafe { core::ptr::read_volatile(ptr) }
    }
}

#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
fn run_tests() {
    log::info!("tlb remote access strong proof requires riscv64 or aarch64");
}

#[cfg(any(target_arch = "riscv64", target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
    remote_access::start(argc, argv)
}

#[cfg(any(target_arch = "riscv64", target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(entry: usize, arg: usize) -> ! {
    // SAFETY: 新线程由调度器切入时中断处于关闭状态，调度锁已在 `switch_to` 前释放。
    unsafe { simplekernel::task::bootstrap_enable_irq() };

    // SAFETY: `entry` 由 `TaskControlBlock::new_kernel_thread` 编码为合法 `fn(usize)`。
    let entry_fn: fn(usize) = unsafe { core::mem::transmute(entry) };
    entry_fn(arg);

    simplekernel::syscall::process::exit(0);
}

#[cfg(any(target_arch = "riscv64", target_arch = "aarch64"))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    test_harness::report_unexpected_panic(info);
    test_harness::exit_qemu(1);
}

#[cfg(any(target_arch = "riscv64", target_arch = "aarch64"))]
#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    simplekernel::logging::raw_put("TEST PANIC: alloc error\n");
    let _ = layout;
    test_harness::exit_qemu(1);
}
