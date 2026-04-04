//! 架构中断控制原语——直接操作 CPU 中断使能位。
//!
//! [`InterruptArch`] 定义中断控制契约，各架构独立实现。
//! `irq_disable` / `irq_enable` 为 `pub(crate)`——外部只能通过
//! [`HeldInterrupts::hold()`](crate::HeldInterrupts::hold) 关中断。

#[cfg(bare_aarch64)]
mod aarch64;
#[cfg(not(bare_metal))]
mod host;
#[cfg(bare_riscv64)]
mod riscv64;

/// 中断控制架构契约。
///
/// 所有方法均为关联函数（无 `self`），因为中断控制是全局的、无状态的。
pub(crate) trait InterruptArch {
    /// 查询当前中断是否启用。
    fn irq_enabled() -> bool;

    /// 禁用中断。
    fn irq_disable();

    /// 启用中断。
    ///
    /// # Safety
    /// 调用方必须确保：
    /// 1. 当前不在持有禁止中断的锁的临界区内
    /// 2. 中断向量表已正确初始化
    /// 3. 栈和上下文状态允许安全地处理中断
    unsafe fn irq_enable();
}

#[cfg(bare_riscv64)]
pub(crate) type Arch = riscv64::Riscv64;

#[cfg(bare_aarch64)]
pub(crate) type Arch = aarch64::Aarch64;

#[cfg(not(bare_metal))]
pub(crate) type Arch = host::Host;

#[cfg(test)]
pub(crate) use host::set_irq_enabled;
