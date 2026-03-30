//! Layer 1b: 中断安全锁——在 [`Mutex`] 之上叠加中断管理和锁序检查。
//!
//! `IrqSafe<R, T>` **组合**（而非复制）`Mutex`，附加：
//! - 获取时禁用中断（`HeldInterrupts` proof token）
//! - per-CPU 锁栈 + 级别检查（死锁预防）
//!
//! 所有 `Deref`/`DerefMut`/锁释放逻辑由内部 `MutexGuard` 提供，不重复实现。

use core::ops::{Deref, DerefMut};

use crate::mutex::{Mutex, MutexGuard};
use crate::raw::{RawLock, RawSpinLock};
use interrupt_state::HeldInterrupts;

/// 用于强制获取顺序的锁级别常量。
///
/// 数值更小的级别必须优先获取。
/// 在持有更高级别锁时获取更低级别的锁会触发 panic。
pub mod lock_level {
    pub const SCHED_LOCK: u8 = 0;
    pub const TASK_TABLE_LOCK: u8 = 1;
    pub const INTERRUPT_THREADS_LOCK: u8 = 2;
    pub const UNCLASSIFIED: u8 = 0xFF;
}

/// 中断安全锁——获取时禁用中断，释放时恢复。
///
/// 内部组合 [`Mutex<R, T>`]，在此之上叠加中断管理和锁序检查。
/// 适用于**中断 handler 也会获取**的锁（如调度锁、控制台锁）。
///
/// # Usage
///
/// ```ignore
/// static MY_LOCK: SpinLockIrq<MyData> = SpinLockIrq::new(MyData::new(), "my_lock");
/// let guard = MY_LOCK.lock();
/// // guard 被丢弃时恢复中断
/// ```
pub struct IrqSafe<R: RawLock, T> {
    mutex: Mutex<R, T>,
    #[cfg_attr(not(target_os = "none"), allow(dead_code))]
    level: u8,
}

// SAFETY: 安全性由内部 Mutex 保证。
unsafe impl<R: RawLock, T: Send> Send for IrqSafe<R, T> {}
unsafe impl<R: RawLock, T: Send> Sync for IrqSafe<R, T> {}

/// `SpinLockIrq<T>` 特化构造器——保持与原 API 完全兼容。
impl<T> IrqSafe<RawSpinLock, T> {
    #[must_use]
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            mutex: Mutex::new(data, name),
            level: lock_level::UNCLASSIFIED,
        }
    }

    #[must_use]
    pub const fn new_with_level(data: T, name: &'static str, level: u8) -> Self {
        Self {
            mutex: Mutex::new(data, name),
            level,
        }
    }
}

/// 泛型方法——适用于所有 `RawLock` 后端。
impl<R: RawLock, T> IrqSafe<R, T> {
    /// 获取锁，返回 RAII guard。
    ///
    /// 使用 [`HeldInterrupts`] 证明令牌管理中断状态：
    /// 获取时禁用中断，guard 析构时恢复。
    ///
    /// # Panics
    /// - 同一核心递归加锁
    /// - 锁级别顺序违反
    pub fn lock(&self) -> IrqSafeGuard<'_, R, T> {
        let held = HeldInterrupts::hold();

        let inner = self.mutex.lock();
        self.post_acquire();

        IrqSafeGuard {
            inner,
            irq_safe: self,
            _held: held,
        }
    }

    /// 尝试获取锁，不阻塞。
    pub fn try_lock(&self) -> Option<IrqSafeGuard<'_, R, T>> {
        let held = HeldInterrupts::hold();

        if let Some(inner) = self.mutex.try_lock() {
            self.post_acquire();
            Some(IrqSafeGuard {
                inner,
                irq_safe: self,
                _held: held,
            })
        } else {
            // held 在此处 drop，自动恢复中断
            None
        }
    }

    /// 嵌套获取——调用者已关中断，用 proof token 证明。
    ///
    /// **有意跳过**中断管理和锁序检查，保留 RAII 自动释放。
    /// 典型场景：任务窃取时在已持有自身调度锁（level 0）的情况下，
    /// `try_lock` 获取另一核心的同级别调度锁——`try_lock` 语义保证不会死锁
    /// （失败立即返回 `None`），因此同级别锁获取是安全的。
    ///
    /// 替代原 `try_lock_raw_no_irq` / `unlock_raw_no_irq` 逃生舱，
    /// 将「中断已禁用」前置条件从 unsafe 注释升级为编译期类型约束。
    pub fn try_lock_nested(&self, _proof: &HeldInterrupts) -> Option<IrqSafeNestedGuard<'_, R, T>> {
        self.mutex
            .try_lock()
            .map(|inner| IrqSafeNestedGuard { inner })
    }

    /// 查询锁是否被持有。
    pub fn is_locked(&self) -> bool {
        self.mutex.is_locked()
    }

    fn post_acquire(&self) {
        #[cfg(target_os = "none")]
        {
            // SAFETY: 中断已禁用，无同核心并发访问
            let stack = unsafe { crate::LOCK_STACK.get_mut() };
            if !stack.check_order(self.level, lock_level::UNCLASSIFIED) {
                panic!(
                    "FATAL: SpinLock '{}': lock order violation",
                    self.mutex.name()
                );
            }
            stack.push(self as *const Self as *const (), self.level);
        }
    }

    #[cfg(target_os = "none")]
    fn pop_lock_stack(&self) {
        // SAFETY: 中断已禁用，无同核心并发访问
        let stack = unsafe { crate::LOCK_STACK.get_mut() };
        stack.pop(self as *const Self as *const ());
    }
}

/// RAII guard（中断安全版）。
///
/// Drop 顺序由 Rust 语言规则保证（[RFC 1857] / [Reference §Destructors]）：
/// 1. 自定义 `Drop::drop()` 执行 → 弹出锁栈
/// 2. `inner` (`MutexGuard`) 析构 → clear_owner + release 锁
/// 3. `_held` (`HeldInterrupts`) 析构 → 恢复中断
///
/// [Reference §Destructors]: https://doc.rust-lang.org/reference/destructors.html
pub struct IrqSafeGuard<'a, R: RawLock, T> {
    inner: MutexGuard<'a, R, T>,
    irq_safe: &'a IrqSafe<R, T>,
    /// 中断禁用的证明令牌——在自定义 Drop 和 inner 之后由编译器析构
    _held: HeldInterrupts,
}

impl<R: RawLock, T> Deref for IrqSafeGuard<'_, R, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<R: RawLock, T> DerefMut for IrqSafeGuard<'_, R, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<R: RawLock, T> Drop for IrqSafeGuard<'_, R, T> {
    fn drop(&mut self) {
        // 弹出锁栈（中断仍禁用，因为 _held 还没 drop）
        #[cfg(target_os = "none")]
        self.irq_safe.pop_lock_stack();

        // irq_safe 字段在 bare-metal 上用于 pop_lock_stack；
        // 非 bare-metal（测试）时未使用，消除 unused 警告
        #[cfg(not(target_os = "none"))]
        let _ = self.irq_safe;

        // inner (MutexGuard) 随后 drop → clear_owner + release
        // _held 最后 drop → 恢复中断
    }
}

/// 嵌套锁 RAII guard——不持有 `HeldInterrupts`（调用者负责中断管理）。
///
/// 由 [`IrqSafe::try_lock_nested`] 返回，在析构时自动释放锁。
pub struct IrqSafeNestedGuard<'a, R: RawLock, T> {
    inner: MutexGuard<'a, R, T>,
}

impl<R: RawLock, T> Deref for IrqSafeNestedGuard<'_, R, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<R: RawLock, T> DerefMut for IrqSafeNestedGuard<'_, R, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

// IrqSafeNestedGuard 无自定义 Drop——直接由 inner (MutexGuard) 的 Drop 释放锁。

#[cfg(test)]
mod tests {
    use super::*;

    type SpinLockIrq<T> = IrqSafe<RawSpinLock, T>;

    /// 基本加锁/解锁流程
    #[test]
    fn irq_lock_and_unlock() {
        let lock = SpinLockIrq::new(42u32, "irq_test");
        {
            let guard = lock.lock();
            assert_eq!(*guard, 42);
        }
        assert!(!lock.is_locked());
    }

    /// try_lock 成功
    #[test]
    fn irq_try_lock() {
        let lock = SpinLockIrq::new(0u32, "irq_try");
        assert!(lock.try_lock().is_some());
    }

    /// try_lock 在锁已持有时失败
    #[test]
    fn irq_try_lock_fails_when_held() {
        let lock = SpinLockIrq::new(0u32, "irq_try_held");
        let _g = lock.lock();
        assert!(lock.try_lock().is_none());
    }

    /// 带锁级别的构造
    #[test]
    fn irq_new_with_level() {
        let lock = SpinLockIrq::new_with_level(0u32, "leveled", lock_level::SCHED_LOCK);
        let _g = lock.lock();
        assert!(lock.is_locked());
    }

    /// 多线程并发访问
    #[test]
    fn irq_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let lock = Arc::new(SpinLockIrq::new(Vec::<usize>::new(), "irq_concurrent"));
        let mut handles = Vec::new();

        for i in 0..4 {
            let lock = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    let mut g = lock.lock();
                    g.push(i * 100 + j);
                }
            }));
        }

        for h in handles {
            h.join().expect("线程应正常结束");
        }

        let g = lock.lock();
        assert_eq!(g.len(), 400, "应有 4×100=400 个元素");
    }

    /// 嵌套锁获取与自动释放
    #[test]
    fn try_lock_nested_and_drop() {
        let lock = SpinLockIrq::new((), "nested_test");
        let held = HeldInterrupts::hold();
        {
            let guard = lock.try_lock_nested(&held);
            assert!(guard.is_some());
            assert!(lock.is_locked());
        }
        assert!(!lock.is_locked());
        drop(held);
    }

    /// 嵌套锁在锁已持有时获取失败
    #[test]
    fn try_lock_nested_fails_when_held() {
        let lock = SpinLockIrq::new((), "nested_fail");
        let _g = lock.lock();
        let held = HeldInterrupts::hold();
        assert!(lock.try_lock_nested(&held).is_none());
    }

    /// 递归加锁应 panic
    #[test]
    #[should_panic(expected = "recursive lock")]
    fn irq_recursive_lock_panics() {
        let lock = SpinLockIrq::new(0u32, "irq_recursive");
        let _g = lock.lock();
        let _g2 = lock.lock();
    }
}
