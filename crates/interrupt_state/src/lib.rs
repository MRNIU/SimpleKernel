//! 中断状态管理——[`HeldInterrupts`] proof token 和架构中断原语。
//!
//! 关中断的唯一公开方式是 [`HeldInterrupts::hold()`]，
//! guard 析构时自动恢复之前的中断状态，杜绝遗漏恢复。
//!
//! [`bootstrap_enable()`] 仅供首次启用中断——它不是"恢复"，
//! 而是"开启"，不与 disable 配对，因此标记为 `unsafe`。

#![cfg_attr(not(test), no_std)]

mod arch;
mod held;

pub use held::HeldInterrupts;

use arch::{Arch, InterruptArch as _};

/// 查询当前中断是否启用。
#[inline(always)]
pub fn is_enabled() -> bool {
    Arch::irq_enabled()
}

/// 首次启用中断——仅供 bootstrap 阶段调用。
///
/// 与 `HeldInterrupts` 的"保存-恢复"模式不同，此函数无条件开启中断，
/// 不关心之前的状态。典型场景：新任务首次获得 CPU 后开启中断。
///
/// # Safety
/// 调用方必须确保：
/// 1. 当前不在持有禁止中断的锁的临界区内
/// 2. 中断向量表已正确初始化
/// 3. 栈和上下文状态允许安全地处理中断
#[inline(always)]
pub unsafe fn bootstrap_enable() {
    // SAFETY: 由调用方保证安全性
    unsafe { Arch::irq_enable() };
}
