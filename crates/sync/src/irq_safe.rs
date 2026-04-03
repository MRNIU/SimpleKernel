//! 中断安全锁——在 [`Mutex`] 之上叠加中断管理和锁序检查。
//!
//! 设计决策详见 `crates/sync/README.md`。

use core::fmt;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};

use crate::lock_stack::lock_level;
use crate::mutex::{Mutex, MutexGuard};
use crate::raw::{RawLock, RawSpinLock};
use interrupt_state::HeldInterrupts;

/// 中断安全锁——获取时禁用中断，释放时恢复。
pub struct IrqSafe<R: RawLock, T> {
    mutex: Mutex<R, T>,
    level: u8,
}

// SAFETY: 安全性由内部 Mutex 保证。
unsafe impl<R: RawLock, T: Send> Send for IrqSafe<R, T> {}
unsafe impl<R: RawLock, T: Send> Sync for IrqSafe<R, T> {}

impl<T> IrqSafe<RawSpinLock, T> {
    /// 创建中断安全锁，默认使用 [`lock_level::CONSOLE`] 级别。
    ///
    /// 生产代码中有明确锁序关系的锁应使用 [`new_with_level`](Self::new_with_level)。
    #[must_use]
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            mutex: Mutex::new(data, name, lock_level::UNSPECIFIED),
            level: lock_level::CONSOLE,
        }
    }

    #[must_use]
    pub const fn new_with_level(data: T, name: &'static str, level: u8) -> Self {
        Self {
            mutex: Mutex::new(data, name, lock_level::UNSPECIFIED),
            level,
        }
    }
}

impl<R: RawLock, T> IrqSafe<R, T> {
    /// 获取锁，返回 RAII guard（loop-try-reopen 模式）。
    ///
    /// # Panics
    /// 同一核心递归加锁，或锁级别顺序违反。
    pub fn lock(&self) -> IrqSafeGuard<'_, R, T> {
        loop {
            let held = HeldInterrupts::hold();

            // 关中断后检测：保证 per-CPU owner_core 与当前核心一致
            self.mutex.raw.check_recursive();

            if let Some(inner) = self.mutex.try_lock() {
                self.post_acquire();
                return IrqSafeGuard {
                    inner: ManuallyDrop::new(inner),
                    irq_safe: self,
                    held: ManuallyDrop::new(held),
                };
            }

            drop(held);

            while self.mutex.raw.is_locked() {
                core::hint::spin_loop();
            }
        }
    }

    /// 尝试获取锁，不阻塞。
    pub fn try_lock(&self) -> Option<IrqSafeGuard<'_, R, T>> {
        if self.mutex.is_locked() {
            return None;
        }

        let held = HeldInterrupts::hold();

        if let Some(inner) = self.mutex.try_lock() {
            self.post_acquire();
            Some(IrqSafeGuard {
                inner: ManuallyDrop::new(inner),
                irq_safe: self,
                held: ManuallyDrop::new(held),
            })
        } else {
            None
        }
    }

    /// 嵌套获取——调用者已关中断，用 proof token 证明。
    ///
    /// 跳过中断管理和锁序检查，保留 RAII 自动释放。
    /// 典型场景：任务窃取时获取另一核心的同级别调度锁。
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
            if !stack.check_order(self.level) {
                panic!(
                    "FATAL: SpinLock '{}': lock order violation (level={})",
                    self.mutex.name(),
                    self.level,
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

impl<R: RawLock, T> fmt::Debug for IrqSafe<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IrqSafe")
            .field("name", &self.mutex.name())
            .field("locked", &self.mutex.is_locked())
            .field("level", &self.level)
            .finish()
    }
}

/// RAII guard（中断安全版）——析构顺序由 `ManuallyDrop` 显式控制。
pub struct IrqSafeGuard<'a, R: RawLock, T> {
    inner: ManuallyDrop<MutexGuard<'a, R, T>>,
    #[cfg_attr(not(target_os = "none"), allow(dead_code))]
    irq_safe: &'a IrqSafe<R, T>,
    held: ManuallyDrop<HeldInterrupts>,
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
        // 1. 弹出锁栈（中断仍禁用，per-CPU 访问安全）
        #[cfg(target_os = "none")]
        self.irq_safe.pop_lock_stack();

        // 2. 释放锁（clear_owner + release）
        // SAFETY: inner 在此之后不再被访问，且仅 drop 一次
        unsafe { ManuallyDrop::drop(&mut self.inner) };

        // 3. 恢复中断（若获取前中断已启用）
        // SAFETY: held 在此之后不再被访问，且仅 drop 一次
        unsafe { ManuallyDrop::drop(&mut self.held) };
    }
}

impl<R: RawLock, T: fmt::Debug> fmt::Debug for IrqSafeGuard<'_, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
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

impl<R: RawLock, T: fmt::Debug> fmt::Debug for IrqSafeNestedGuard<'_, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
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
        let lock = SpinLockIrq::new_with_level(0u32, "leveled", lock_level::SCHED);
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
        assert_eq!(g.len(), 400, "应有 4x100=400 个元素");
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
