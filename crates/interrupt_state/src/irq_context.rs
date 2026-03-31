//! 中断上下文跟踪——RAII 守卫 + per-CPU 嵌套计数。
//!
//! 通过 [`HardIrqGuard`] 将"处于硬中断上下文"编码为类型约束，
//! 创建时递增 per-CPU 计数，析构时递减，杜绝遗漏配对。
//!
//! ## Per-CPU 状态
//!
//! | 变量 | 类型 | 说明 |
//! |------|------|------|
//! | `HARDIRQ_COUNT` | `AtomicU32` | 硬中断嵌套深度 |
//! | `SOFTIRQ_COUNT` | `AtomicU32` | 软中断嵌套深度 |

use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

use per_cpu::cpu_local;

/// 硬中断嵌套计数（>0 表示在 hardirq 上下文中）
#[cpu_local]
static HARDIRQ_COUNT: AtomicU32 = AtomicU32::new(0);

/// 软中断嵌套计数（>0 表示在 softirq 上下文中）
// TODO: 引入 SoftIrqGuard 后启用；当前始终为 0
#[cpu_local]
static SOFTIRQ_COUNT: AtomicU32 = AtomicU32::new(0);

/// 硬中断上下文的 RAII 守卫。
///
/// 创建时递增 per-CPU `HARDIRQ_COUNT`，析构时递减。
/// 不可 Clone / Copy / Send —— 中断上下文是 per-CPU 的，
/// 且必须在同一核心上成对进出。
///
/// # Examples
///
/// ```ignore
/// let _irq = HardIrqGuard::enter();
/// // 此作用域内 in_interrupt() 返回 true
/// handle_timer_tick();
/// // drop 时自动递减计数
/// ```
pub struct HardIrqGuard {
    /// `*const ()` 使类型自动 `!Send + !Sync`——中断上下文是 per-CPU 的，不可跨核传递。
    _not_send: PhantomData<*const ()>,
}

impl HardIrqGuard {
    /// 进入硬中断上下文（递增计数），返回守卫。
    ///
    /// 必须在中断处理程序中调用（中断已被 CPU 自动关闭）。
    #[must_use]
    pub fn enter() -> Self {
        HARDIRQ_COUNT.get().fetch_add(1, Ordering::Relaxed);
        Self {
            _not_send: PhantomData,
        }
    }
}

impl Drop for HardIrqGuard {
    fn drop(&mut self) {
        HARDIRQ_COUNT.get().fetch_sub(1, Ordering::Relaxed);
    }
}

/// 当前是否处于中断上下文（不可调度）。
pub fn in_interrupt() -> bool {
    HARDIRQ_COUNT.get().load(Ordering::Relaxed) > 0
        || SOFTIRQ_COUNT.get().load(Ordering::Relaxed) > 0
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// 串行化访问全局 per-CPU 变量，防止并行测试相互干扰。
    static IRQ_CTX_LOCK: Mutex<()> = Mutex::new(());

    /// 默认状态不在中断上下文中。
    #[test]
    fn not_in_interrupt_by_default() {
        let _guard = IRQ_CTX_LOCK.lock().expect("IRQ_CTX_LOCK poisoned");
        assert!(!in_interrupt());
    }

    /// HardIrqGuard 进入/退出正确更新状态。
    #[test]
    fn hard_irq_guard_enter_exit() {
        let _guard = IRQ_CTX_LOCK.lock().expect("IRQ_CTX_LOCK poisoned");
        assert!(!in_interrupt());
        let irq = HardIrqGuard::enter();
        assert!(in_interrupt());
        drop(irq);
        assert!(!in_interrupt());
    }

    /// 嵌套 HardIrqGuard 正确处理。
    #[test]
    fn nested_hard_irq_guard() {
        let _guard = IRQ_CTX_LOCK.lock().expect("IRQ_CTX_LOCK poisoned");
        let outer = HardIrqGuard::enter();
        assert!(in_interrupt());
        let inner = HardIrqGuard::enter();
        assert!(in_interrupt());
        drop(inner);
        assert!(in_interrupt());
        drop(outer);
        assert!(!in_interrupt());
    }

    /// HardIrqGuard 大小为 0（只有 PhantomData）。
    #[test]
    fn hard_irq_guard_is_zst() {
        assert_eq!(core::mem::size_of::<HardIrqGuard>(), 0);
    }
}
