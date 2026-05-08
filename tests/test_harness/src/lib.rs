//! 独立 QEMU 测试二进制的公共 harness。
//!
//! 提供 `test_main!` 宏，消除每个测试二进制的样板代码
//! （`_start`、`panic_handler`、`exit_qemu`、`kernel_thread_bootstrap`）。

#![no_std]

extern crate alloc;

/// 退出 QEMU，`code` 为退出码（0 = 成功，非零 = 失败）。
pub fn exit_qemu(code: u32) -> ! {
    #[cfg(target_arch = "riscv64")]
    {
        let _ = code;
        sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
        loop {
            core::hint::spin_loop();
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        let _ = code;
        // SAFETY: PSCI SYSTEM_OFF 是标准固件接口
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

/// 生成测试二进制的入口和样板代码。
///
/// # 普通测试
///
/// `test_fn` 正常返回 → QEMU 退出码 0（成功），panic → handle_panic 打印栈回溯后挂起（失败）。
///
/// ```ignore
/// test_harness::test_main!(simplekernel::boot::InitLevel::Full, test_fn);
///
/// fn test_fn() {
///     assert_eq!(1 + 1, 2);
/// }
/// ```
///
/// # should_panic 测试
///
/// `test_fn` panic → QEMU 退出码 0（成功），正常返回 → 退出码 1（失败）。
///
/// ```ignore
/// test_harness::test_main!(simplekernel::boot::InitLevel::Memory, test_fn, should_panic);
///
/// fn test_fn() {
///     panic!("expected panic");
/// }
/// ```
#[macro_export]
macro_rules! test_main {
    ($level:expr, $test_fn:ident) => {
        static PRIMARY_BOOTED: core::sync::atomic::AtomicBool =
            core::sync::atomic::AtomicBool::new(false);

        #[unsafe(no_mangle)]
        pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
            if !PRIMARY_BOOTED.swap(true, core::sync::atomic::Ordering::AcqRel) {
                // SAFETY: bare-metal 环境，主核首次调用
                unsafe {
                    simplekernel::boot::kernel_init(argc, argv, $level);
                }
                $test_fn();
                log::info!("TEST OK");
                $crate::exit_qemu(0);
            } else {
                // SAFETY: 从核入口，汇编已设置栈和寄存器
                unsafe { simplekernel::boot::kernel_init_smp() };
                loop {
                    core::hint::spin_loop();
                }
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn kernel_thread_bootstrap(_entry: usize, _arg: usize) -> ! {
            loop {
                core::hint::spin_loop();
            }
        }

        #[panic_handler]
        fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
            simplekernel::panic::handle_panic(info);
        }

        #[alloc_error_handler]
        fn alloc_error(layout: core::alloc::Layout) -> ! {
            simplekernel::logging::raw_put("TEST PANIC: alloc error\n");
            let _ = layout;
            $crate::exit_qemu(1);
        }
    };

    ($level:expr, $test_fn:ident, should_panic) => {
        static PRIMARY_BOOTED: core::sync::atomic::AtomicBool =
            core::sync::atomic::AtomicBool::new(false);

        #[unsafe(no_mangle)]
        pub extern "C" fn _start(argc: i32, argv: *const *const u8) -> ! {
            if !PRIMARY_BOOTED.swap(true, core::sync::atomic::Ordering::AcqRel) {
                // SAFETY: bare-metal 环境，主核首次调用
                unsafe {
                    simplekernel::boot::kernel_init(argc, argv, $level);
                }
                $test_fn();
                // 执行到此说明没有 panic → should_panic 测试失败
                simplekernel::logging::raw_put("SHOULD_PANIC test returned without panic\n");
                $crate::exit_qemu(1);
            } else {
                // SAFETY: 从核入口，汇编已设置栈和寄存器
                unsafe { simplekernel::boot::kernel_init_smp() };
                loop {
                    core::hint::spin_loop();
                }
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn kernel_thread_bootstrap(_entry: usize, _arg: usize) -> ! {
            loop {
                core::hint::spin_loop();
            }
        }

        #[panic_handler]
        fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
            if let Some(loc) = info.location() {
                log::info!(
                    "SHOULD_PANIC OK: {}:{}: {}",
                    loc.file(),
                    loc.line(),
                    info.message()
                );
            } else {
                log::info!("SHOULD_PANIC OK: <unknown>: {}", info.message());
            }
            // panic 发生 = should_panic 测试成功
            $crate::exit_qemu(0);
        }

        #[alloc_error_handler]
        fn alloc_error(layout: core::alloc::Layout) -> ! {
            simplekernel::logging::raw_put("TEST PANIC: alloc error\n");
            let _ = layout;
            $crate::exit_qemu(1);
        }
    };
}
