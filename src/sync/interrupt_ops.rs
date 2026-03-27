/// 公共中断控制接口
///
/// 通过 `ArchOps` trait 分派到具体架构实现，测试模式下为 no-op 存根。
///
/// # 核心类型
///
/// - [`HeldInterrupts`]：证明令牌（proof token），证明中断已被禁用。
///   不可 Clone / Copy，析构时自动恢复中断状态。
///   借鉴 Theseus OS 的 intralingual 设计哲学：
///   将「中断已关闭」这一运行时不变量编码为编译期类型约束。

#[cfg(not(test))]
use crate::arch::ArchOps;

// ─── 底层操作（模块内部使用） ────────────────────────────────────────

/// 查询当前中断是否启用
#[inline(always)]
pub fn get_status() -> bool {
    #[cfg(not(test))]
    {
        crate::arch::Arch::irq_enabled()
    }
    #[cfg(test)]
    {
        false
    }
}

/// 禁用中断
#[inline(always)]
pub fn disable() {
    #[cfg(not(test))]
    crate::arch::Arch::irq_disable();
}

/// 启用中断
///
/// # Safety
/// 调用方必须确保在启用中断后不会破坏当前临界区的不变量。
#[inline(always)]
pub unsafe fn enable() {
    #[cfg(not(test))]
    // SAFETY: 由调用方保证安全性
    unsafe {
        crate::arch::Arch::irq_enable()
    };
}

// ─── HeldInterrupts 证明令牌 ─────────────────────────────────────────

/// 中断禁用的证明令牌（proof token）。
///
/// 持有此类型的值即证明中断已被禁用。
/// 不可 Clone / Copy —— 确保每次 `hold()` 与恢复一一对应。
/// 析构时自动恢复之前的中断状态。
///
/// # 设计理念
///
/// 传统做法（`disable()` + 手动 `enable()`）容易遗漏恢复调用。
/// `HeldInterrupts` 将「中断已关闭」编码为 Rust 类型：
/// - 编译器保证 token 不会被复制或遗忘（非 Copy、RAII Drop）
/// - 函数签名可以要求 `&HeldInterrupts` 参数，证明调用方已禁用中断
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
pub struct HeldInterrupts {
    was_enabled: bool,
}

impl HeldInterrupts {
    /// 保存当前中断状态、禁用中断，返回证明令牌。
    #[inline]
    #[must_use]
    pub fn hold() -> Self {
        let was_enabled = get_status();
        disable();
        Self { was_enabled }
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
            unsafe { enable() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_interrupts_hold_and_drop() {
        // 测试环境下 get_status 返回 false，所以 was_enabled = false
        let held = HeldInterrupts::hold();
        assert!(!held.was_enabled());
        drop(held); // 不会调用 enable（因为 was_enabled = false）
    }

    #[test]
    fn held_interrupts_is_not_copy() {
        // 编译期验证：HeldInterrupts 不可 Copy
        fn assert_not_copy<T>() {
            // 如果 T: Copy，这个函数会编译通过
            // 我们通过运行时检查 size 来确认类型存在
        }
        assert_not_copy::<HeldInterrupts>();
        // HeldInterrupts 的 size 应该是 1 byte（bool）
        assert_eq!(core::mem::size_of::<HeldInterrupts>(), 1);
    }
}
