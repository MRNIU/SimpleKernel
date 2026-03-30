//! SimpleKernel 系统测试内核——独立二进制，运行在 QEMU 中。
//!
//! 引导序列：`_start` → `kernel_init(Full)` → 构建 TestRunner → 运行测试 → 退出 QEMU。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

mod framework;
mod memory_tests;
mod sync_tests;

use core::alloc::Layout;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

use framework::{TestGroup, TestRunner};

/// 标记主核是否已完成初始化，用于区分主核/从核引导路径
static PRIMARY_BOOTED: AtomicBool = AtomicBool::new(false);

/// 测试内核入口点
///
/// 主核进入 `test_main`，从核进入 `test_smp`。
#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
    // swap 返回旧值：false → 当前核是第一个到达的核（主核）
    if !PRIMARY_BOOTED.swap(true, Ordering::AcqRel) {
        test_main(argc, argv);
    } else {
        test_smp();
    }
}

/// 内核线程引导函数（供 switch.S 中 `kernel_thread_entry` 调用）
///
/// 测试内核中也需要定义此符号，因为 switch.S 引用了它。
#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(_entry: usize, _arg: usize) -> ! {
    // 测试内核不创建内核线程，此处仅为满足链接需求
    loop {
        core::hint::spin_loop();
    }
}

/// 主核测试入口
fn test_main(argc: i32, argv: *const *const u8) -> ! {
    // SAFETY: bare-metal 环境，主核首次调用
    unsafe {
        simplekernel::boot::kernel_init(argc, argv, simplekernel::boot::InitLevel::Full);
    }

    // 构建测试运行器并添加测试组
    let mut runner = TestRunner::new();

    runner.add_group(TestGroup {
        name: "memory",
        tests: memory_tests::tests(),
    });

    runner.add_group(TestGroup {
        name: "sync",
        tests: sync_tests::tests(),
    });

    // 运行所有测试
    let all_passed = runner.run();

    // 根据测试结果退出 QEMU
    if all_passed {
        exit_qemu(0);
    } else {
        exit_qemu(1);
    }
}

/// 从核入口——初始化后进入空转循环
fn test_smp() -> ! {
    // SAFETY: 从核入口，汇编已设置栈和寄存器
    unsafe {
        simplekernel::boot::kernel_init_smp();
    }

    // 从核上线后尝试调度
    simplekernel::task::schedule();

    // 空转循环
    loop {
        if simplekernel::preempt::check_and_clear_need_resched() {
            simplekernel::task::schedule();
        }
        core::hint::spin_loop();
    }
}

/// 退出 QEMU，`code` 为退出码（0 = 成功，非零 = 失败）
fn exit_qemu(code: u32) -> ! {
    #[cfg(target_arch = "riscv64")]
    {
        let _ = code;
        sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
        // 如果 SBI 调用未生效，回退到无限循环
        loop {
            core::hint::spin_loop();
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        use qemu_exit::QEMUExit;
        let handle = qemu_exit::AArch64::new();
        handle.exit(code);
    }

    #[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
    {
        let _ = code;
        loop {
            core::hint::spin_loop();
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    // bare-metal 环境无法捕获 panic，直接终止测试内核。
    // handle_panic 会输出 backtrace 后进入无限循环。
    simplekernel::panic::handle_panic(info);
}

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    simplekernel::logging::raw_put("SYSTEM TEST PANIC: alloc error\n");
    let _ = layout;
    exit_qemu(1);
}
