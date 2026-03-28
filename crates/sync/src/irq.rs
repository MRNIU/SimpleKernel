//! 架构中断控制原语——直接操作 CPU 中断使能位。
//!
//! 使用 `target_os = "none"` 区分裸机（内核）和宿主机（测试/clippy）：
//! - `cfg(target_os = "none")`: 裸机编译，执行真实的特权操作
//! - `cfg(not(target_os = "none"))`: 宿主机编译，no-op 或 mock

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use aarch64_cpu::registers::{DAIF, Readable};

/// 查询当前中断是否启用。
#[inline(always)]
pub fn irq_enabled() -> bool {
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
#[inline(always)]
pub fn irq_disable() {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    riscv::interrupt::supervisor::disable();
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    // SAFETY: daifset 是 EL1 特权指令，原子地设置 DAIF 位。
    // 不能使用 DAIF.write()（msr DAIF, Xn），因为它会覆盖整个寄存器，
    // 可能意外取消屏蔽 Debug/SError/FIQ 异常。
    unsafe {
        core::arch::asm!("msr daifset, #2");
    }
    // 宿主机: no-op
}

/// 启用中断。
///
/// # Safety
/// 调用方必须确保：
/// 1. 当前不在持有禁止中断的 `SpinLock` 的临界区内
/// 2. 中断向量表已正确初始化（`init_interrupt()` 已完成）
/// 3. 栈和上下文状态允许安全地处理中断
#[inline(always)]
pub unsafe fn irq_enable() {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    // SAFETY: 由调用方保证安全性
    unsafe {
        riscv::interrupt::supervisor::enable()
    };
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    // SAFETY: daifclr 是 EL1 特权指令，原子地清除 DAIF 位。
    // 同 irq_disable，不能使用 DAIF.write()。
    unsafe {
        core::arch::asm!("msr daifclr, #2");
    }
    // 宿主机: no-op
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证宿主机上中断状态恒为 disabled。
    #[test]
    fn irq_disabled_on_host() {
        assert!(!irq_enabled());
    }

    /// 验证宿主机上 irq_disable 不会 panic 或 SIGILL。
    #[test]
    fn irq_disable_is_noop_on_host() {
        irq_disable();
    }
}
