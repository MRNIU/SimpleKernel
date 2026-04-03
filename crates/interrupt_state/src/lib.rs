//! 中断状态管理——proof token、RAII 守卫和架构中断原语。
//!
//! | 类型 | 作用 |
//! |------|------|
//! | [`HeldInterrupts`] | 关中断的 proof token，析构时恢复 |
//! | [`HardIrqGuard`] | 标记硬中断上下文，析构时递减计数 |
//! | [`is_in_interrupt()`] | 查询是否处于中断上下文 |
//!
//! 关中断的唯一公开方式是 [`HeldInterrupts::hold()`]，
//! guard 析构时自动恢复之前的中断状态，杜绝遗漏恢复。
//!
//! [`bootstrap_enable()`] 仅供首次启用中断——它不是"恢复"，
//! 而是"开启"，不与 disable 配对，因此标记为 `unsafe`。

#![cfg_attr(not(test), no_std)]

mod arch;

use arch::{Arch, InterruptArch as _};
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use per_cpu::cpu_local;

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

/// 中断禁用的证明令牌（proof token）。
///
/// 持有此类型的值即证明中断已被禁用。
/// 不可 Clone / Copy / Send —— 确保每次 `hold()` 与恢复一一对应，
/// 且不可跨核心传递（一个核心上保存的中断状态在另一核心上恢复是错误的）。
/// 析构时自动恢复之前的中断状态。
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
/// use interrupt_state::HeldInterrupts;
/// let a = HeldInterrupts::hold();
/// let b = a;
/// drop(a); // 不应编译通过
/// ```
///
/// HeldInterrupts 不可 Send（不可跨线程传递）：
/// ```compile_fail
/// use interrupt_state::HeldInterrupts;
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
        let was_enabled = Arch::irq_enabled();
        Arch::irq_disable();
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
            unsafe { Arch::irq_enable() };
        }
    }
}

/// 硬中断嵌套计数（>0 表示在 hardirq 上下文中）
#[cpu_local]
static HARDIRQ_COUNT: AtomicU32 = AtomicU32::new(0);

/// 软中断嵌套计数（>0 表示在 softirq 上下文中）
// TODO: 引入 SoftIrqGuard 后启用；当前始终为 0
#[cpu_local]
static SOFTIRQ_COUNT: AtomicU32 = AtomicU32::new(0);

/// 抢占禁用嵌套计数（per-CPU）。
///
/// 大于 0 表示当前核心不可被抢占。由 [`PreemptGuard`] 的构造/析构维护。
#[cpu_local]
static PREEMPT_DISABLE_COUNT: AtomicU32 = AtomicU32::new(0);

/// 重调度标志（per-CPU）。
///
/// 为 `true` 表示当前核心需要在下一个抢占点执行调度。
/// 可由其他核心通过 [`set_need_resched_on()`] 跨核设置。
#[cpu_local]
pub static NEED_RESCHED: AtomicBool = AtomicBool::new(false);

/// 抢占禁用的证明令牌（proof token）。
///
/// 持有此类型的值即证明当前核心的抢占已被禁用。
/// 不可 Clone / Copy / Send —— 抢占状态是 per-CPU 的，
/// 不可跨核心传递。析构时自动递减嵌套计数。
///
/// 类似 C/C++ 中手动调用 `preempt_disable()` / `preempt_enable()` 对，
/// 但利用 Rust RAII 保证一定成对出现，不可能遗漏 `preempt_enable()`。
///
/// # Examples
///
/// ```ignore
/// let guard = PreemptGuard::disable();
/// // 抢占已禁用，可安全操作 per-CPU 数据
/// do_something();
/// drop(guard); // 恢复抢占
/// ```
///
/// PreemptGuard 不可 Send（不可跨线程传递）：
/// ```compile_fail
/// use interrupt_state::PreemptGuard;
/// fn assert_send<T: Send>() {}
/// assert_send::<PreemptGuard>(); // 不应编译通过
/// ```
pub struct PreemptGuard {
    /// `*const ()` 使类型自动 `!Send + !Sync`——抢占状态是 per-CPU 的，不可跨核传递。
    _not_send: PhantomData<*const ()>,
}

impl PreemptGuard {
    /// 禁用抢占（递增嵌套计数），返回守卫。
    ///
    /// 支持嵌套调用——每次调用递增计数，全部守卫析构后才恢复可抢占状态。
    #[inline]
    #[must_use]
    pub fn disable() -> Self {
        PREEMPT_DISABLE_COUNT.get().fetch_add(1, Ordering::Relaxed);
        Self {
            _not_send: PhantomData,
        }
    }
}

impl Drop for PreemptGuard {
    fn drop(&mut self) {
        PREEMPT_DISABLE_COUNT.get().fetch_sub(1, Ordering::Relaxed);
    }
}

/// 当前核心是否可被抢占。
///
/// 仅当抢占禁用计数为 0 **且**不在中断上下文中时返回 `true`。
#[inline]
pub fn preemptible() -> bool {
    PREEMPT_DISABLE_COUNT.get().load(Ordering::Relaxed) == 0 && !is_in_interrupt()
}

/// 原子地检查并清除当前核心的重调度标志。
///
/// 返回清除前的值——`true` 表示确实需要重调度。
/// 使用 `Acquire` 序与 [`set_need_resched_on()`] 的 `Release` 配对，
/// 确保看到设置方在 store 之前的所有写入。
#[inline]
pub fn check_and_clear_need_resched() -> bool {
    NEED_RESCHED.get().swap(false, Ordering::Acquire)
}

/// 设置目标核心的重调度标志。
///
/// 典型场景：唤醒一个任务后，通知其所在核心尽快调度。
/// 使用 `Release` 序确保唤醒操作的写入在目标核心读取标志时可见。
///
/// # Safety
/// 调用方必须确保 `target_core` 是有效的核心编号（< 实际核心数），
/// 否则将访问越界的 per-CPU 数据。
#[inline]
pub unsafe fn set_need_resched_on(target_core: usize) {
    // SAFETY: 调用方保证 target_core 是有效核心编号
    unsafe {
        NEED_RESCHED
            .get_on(target_core)
            .store(true, Ordering::Release);
    }
}

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
/// // 此作用域内 is_in_interrupt() 返回 true
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
pub fn is_in_interrupt() -> bool {
    HARDIRQ_COUNT.get().load(Ordering::Relaxed) > 0
        || SOFTIRQ_COUNT.get().load(Ordering::Relaxed) > 0
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use arch::set_irq_enabled;

    /// 串行化访问全局状态，防止并行测试相互干扰。
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// 中断关闭时 hold：was_enabled=false，drop 后中断仍为关闭。
    #[test]
    fn hold_when_disabled() {
        let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
        set_irq_enabled(false);
        let held = HeldInterrupts::hold();
        assert!(!held.was_enabled());
        assert!(!Arch::irq_enabled());
        drop(held);
        assert!(!Arch::irq_enabled());
    }

    /// 中断开启时 hold：was_enabled=true，hold 期间中断关闭，drop 后恢复开启。
    #[test]
    fn hold_when_enabled_restores_on_drop() {
        let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
        set_irq_enabled(true);
        let held = HeldInterrupts::hold();
        assert!(held.was_enabled());
        assert!(!Arch::irq_enabled());
        drop(held);
        assert!(Arch::irq_enabled());
    }

    /// 验证 HeldInterrupts 的 size（编译期 !Copy / !Send 由 doc test 保证）。
    #[test]
    fn held_interrupts_size() {
        assert_eq!(core::mem::size_of::<HeldInterrupts>(), 1);
    }

    /// 默认状态不在中断上下文中。
    #[test]
    fn not_in_interrupt_by_default() {
        let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
        assert!(!is_in_interrupt());
    }

    /// HardIrqGuard 进入/退出正确更新状态。
    #[test]
    fn hard_irq_guard_enter_exit() {
        let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
        assert!(!is_in_interrupt());
        let irq = HardIrqGuard::enter();
        assert!(is_in_interrupt());
        drop(irq);
        assert!(!is_in_interrupt());
    }

    /// 嵌套 HardIrqGuard 正确处理。
    #[test]
    fn nested_hard_irq_guard() {
        let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
        let outer = HardIrqGuard::enter();
        assert!(is_in_interrupt());
        let inner = HardIrqGuard::enter();
        assert!(is_in_interrupt());
        drop(inner);
        assert!(is_in_interrupt());
        drop(outer);
        assert!(!is_in_interrupt());
    }

    /// HardIrqGuard 大小为 0（只有 PhantomData）。
    #[test]
    fn hard_irq_guard_is_zst() {
        assert_eq!(core::mem::size_of::<HardIrqGuard>(), 0);
    }

    /// PreemptGuard 大小为 0（只有 PhantomData）。
    #[test]
    fn preempt_guard_is_zst() {
        assert_eq!(core::mem::size_of::<PreemptGuard>(), 0);
    }

    /// 禁用抢占后 preemptible() 返回 false，drop 后恢复。
    #[test]
    fn preempt_disable_makes_non_preemptible() {
        let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
        assert!(preemptible());
        let preempt = PreemptGuard::disable();
        assert!(!preemptible());
        drop(preempt);
        assert!(preemptible());
    }

    /// 嵌套 PreemptGuard：内层 drop 后仍不可抢占，外层 drop 后恢复。
    #[test]
    fn nested_preempt_guard() {
        let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
        assert!(preemptible());
        let outer = PreemptGuard::disable();
        assert!(!preemptible());
        let inner = PreemptGuard::disable();
        assert!(!preemptible());
        drop(inner);
        assert!(!preemptible());
        drop(outer);
        assert!(preemptible());
    }

    /// 中断上下文中不可抢占（即使抢占计数为 0）。
    #[test]
    fn not_preemptible_in_interrupt() {
        let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
        assert!(preemptible());
        let irq = HardIrqGuard::enter();
        assert!(!preemptible());
        drop(irq);
        assert!(preemptible());
    }

    /// check_and_clear_need_resched：初始为 false，设置后返回 true，再次检查返回 false。
    #[test]
    fn check_and_clear_need_resched_works() {
        let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
        assert!(!check_and_clear_need_resched());
        NEED_RESCHED.get().store(true, Ordering::Release);
        assert!(check_and_clear_need_resched());
        assert!(!check_and_clear_need_resched());
    }
}
