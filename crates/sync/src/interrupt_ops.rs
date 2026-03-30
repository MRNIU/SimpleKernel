//! 中断证明令牌——将「中断已关闭」编码为编译期类型约束。
//!
//! 借鉴 Theseus OS 的 intralingual 设计哲学：
//! 持有 [`HeldInterrupts`] 即证明中断已被禁用，
//! 函数签名可以要求 `&HeldInterrupts` 参数来强制调用方先关中断。

use core::marker::PhantomData;

use crate::irq;

/// 中断禁用的证明令牌（proof token）。
///
/// 持有此类型的值即证明中断已被禁用。
/// 不可 Clone / Copy / Send —— 确保每次 `hold()` 与恢复一一对应，
/// 且不可跨核心传递（一个核心上保存的中断状态在另一核心上恢复是错误的）。
/// 析构时自动恢复之前的中断状态。
///
/// # 设计理念
///
/// 传统做法（`disable()` + 手动 `enable()`）容易遗漏恢复调用。
/// `HeldInterrupts` 将「中断已关闭」编码为 Rust 类型：
/// - 编译器保证 token 不会被复制或遗忘（非 Copy、RAII Drop）
/// - 函数签名可以要求 `&HeldInterrupts` 参数，证明调用方已禁用中断
/// - `!Send` 防止跨核心传递——中断状态是 per-CPU 的
/// - 与 Theseus OS 的 `HeldInterrupts` 概念一致
///
/// # Examples
///
/// ```ignore
/// let held = HeldInterrupts::hold();
/// // 中断已禁用，可安全访问 per-CPU 数据
/// do_critical_section(&held);
/// drop(held); // 恢复中断
/// ```
///
/// HeldInterrupts 不可 Copy（use-after-move）：
/// ```compile_fail
/// use sync::interrupt_ops::HeldInterrupts;
/// let a = HeldInterrupts::hold();
/// let b = a;
/// drop(a); // 不应编译通过
/// ```
///
/// HeldInterrupts 不可 Send（不可跨线程传递）：
/// ```compile_fail
/// use sync::interrupt_ops::HeldInterrupts;
/// fn assert_send<T: Send>() {}
/// assert_send::<HeldInterrupts>(); // 不应编译通过
/// ```
pub struct HeldInterrupts {
    was_enabled: bool,
    /// `*const ()` 使类型自动 `!Send + !Sync`——中断状态是 per-CPU 的，不可跨核传递。
    _not_send: PhantomData<*const ()>,
}

impl HeldInterrupts {
    /// 保存当前中断状态、禁用中断，返回证明令牌。
    #[inline]
    #[must_use]
    pub fn hold() -> Self {
        let was_enabled = irq::irq_enabled();
        irq::irq_disable();
        Self {
            was_enabled,
            _not_send: PhantomData,
        }
    }

    /// 查询获取令牌前中断是否已启用。
    #[inline]
    #[must_use]
    pub fn was_enabled(&self) -> bool {
        self.was_enabled
    }
}

impl Drop for HeldInterrupts {
    fn drop(&mut self) {
        if self.was_enabled {
            // SAFETY: 恢复到获取令牌前的中断状态
            unsafe { irq::irq_enable() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 hold 保存状态并在 drop 时恢复。
    #[test]
    fn held_interrupts_hold_and_drop() {
        // 测试环境下 irq_enabled 返回 false，所以 was_enabled = false
        let held = HeldInterrupts::hold();
        assert!(!held.was_enabled());
        drop(held); // 不会调用 enable（因为 was_enabled = false）
    }

    /// 验证 HeldInterrupts 的 size（编译期 !Copy / !Send 由 doc test 保证）。
    #[test]
    fn held_interrupts_size() {
        // PhantomData<*const ()> 是 ZST，HeldInterrupts 仍为 1 byte（bool）
        assert_eq!(core::mem::size_of::<HeldInterrupts>(), 1);
    }
}
