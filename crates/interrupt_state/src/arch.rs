//! 架构中断控制原语——直接操作 CPU 中断使能位。
//!
//! `irq_disable` 为 `pub(crate)`——外部只能通过 `HeldInterrupts::hold()` 关中断。

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use aarch64_cpu::registers::{DAIF, Readable};

/// 查询当前中断是否启用。
#[inline(always)]
pub(crate) fn irq_enabled() -> bool {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    {
        riscv::register::sstatus::read().sie()
    }
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    {
        // DAIF.I = 0 表示 IRQ 未屏蔽（即中断启用）
        DAIF.read(DAIF::I) == 0
    }
    #[cfg(not(target_os = "none"))]
    {
        false // 宿主机——中断概念不适用
    }
}

/// 禁用中断。
///
/// `pub(crate)` —— 外部不可直接调用，只能通过 `HeldInterrupts::hold()`。
#[inline(always)]
pub(crate) fn irq_disable() {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    riscv::interrupt::supervisor::disable();
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    // SAFETY: daifset 是 EL1 特权指令，原子地设置 DAIF 位。
    // 不能使用 DAIF.write()（msr DAIF, Xn），因为它会覆盖整个寄存器，
    // 可能意外取消屏蔽 Debug/SError/FIQ 异常。
    // TODO: aarch64-cpu crate 未封装 daifset/daifclr，待上游提供后替换裸汇编
    unsafe {
        core::arch::asm!("msr daifset, #2");
    }
    // 宿主机: no-op
}

/// 启用中断。
///
/// # Safety
/// 由 `crate::irq_enable()` 的调用方保证安全性。
#[inline(always)]
pub(crate) unsafe fn irq_enable() {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    // SAFETY: 由调用方保证安全性
    unsafe {
        riscv::interrupt::supervisor::enable()
    };
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    // SAFETY: daifclr 是 EL1 特权指令，原子地清除 DAIF 位。
    // TODO: 同 irq_disable，待 aarch64-cpu 封装 daifclr 后替换
    unsafe {
        core::arch::asm!("msr daifclr, #2");
    }
    // 宿主机: no-op
}
