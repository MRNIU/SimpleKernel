//! 独立测试：验证 panic handler 正确触发
//!
//! 期望行为：触发 panic → 输出 "PANIC_TEST_TRIGGERED" → 退出 QEMU。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::alloc::Layout;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

/// 标记主核是否已完成初始化，用于区分主核/从核引导路径
static PRIMARY_BOOTED: AtomicBool = AtomicBool::new(false);

/// 测试入口点
///
/// 主核进入 `panic_test_main`，从核进入空转循环。
#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
    // swap 返回旧值：false → 当前核是第一个到达的核（主核）
    if !PRIMARY_BOOTED.swap(true, Ordering::AcqRel) {
        panic_test_main(argc, argv);
    } else {
        loop {
            core::hint::spin_loop();
        }
    }
}

/// 内核线程引导函数（供 switch.S 中 `kernel_thread_entry` 调用）
///
/// 此测试不创建内核线程，仅为满足链接需求。
#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(_entry: usize, _arg: usize) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// 主核测试入口——初始化到 Memory 级别后触发 panic
fn panic_test_main(argc: i32, argv: *const *const u8) -> ! {
    // SAFETY: bare-metal 环境，主核首次调用
    unsafe {
        simplekernel::boot::kernel_init(argc, argv, simplekernel::boot::InitLevel::Memory);
    }

    log::info!("PANIC_TEST: about to trigger intentional panic...");
    panic!("PANIC_TEST_TRIGGERED: this panic is intentional");
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
        // PSCI SYSTEM_OFF (SMC32: 0x84000008)——QEMU virt + ATF 的 conduit 是 SMC。
        let _ = code;
        // SAFETY: PSCI SYSTEM_OFF 是标准固件接口，无参数，关闭整个系统
        unsafe {
            core::arch::asm!(
                "smc #0",
                in("x0") 0x8400_0008u64,
                in("x1") 0u64,
                in("x2") 0u64,
                in("x3") 0u64,
                options(nomem, nostack),
            );
        }
        loop {
            core::hint::spin_loop();
        }
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
fn panic(_info: &PanicInfo<'_>) -> ! {
    // 打印触发标记——测试运行器通过此字符串判断测试通过
    log::info!("SHOULD_PANIC OK: PANIC_TEST_TRIGGERED");
    // 成功退出——触发 panic 就是期望行为
    exit_qemu(0);
}

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    simplekernel::logging::raw_put("TEST PANIC: alloc error\n");
    let _ = layout;
    exit_qemu(1);
}
