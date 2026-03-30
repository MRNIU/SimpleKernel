//! Layer 1a: 泛型互斥锁——数据保护 + RAII guard。
//!
//! `Mutex<R, T>` 将任意 [`RawLock`] 实现与受保护数据组合，
//! `MutexGuard` 提供 RAII 生命周期管理。
//! `Deref` / `DerefMut` / `Drop` 只需实现一次，所有锁后端共享。

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

use crate::raw::{RawLock, RawSpinLock};

/// 泛型互斥锁——通过 `R: RawLock` 参数化底层锁算法。
///
/// 不涉及中断管理。如果中断 handler 也会获取同一把锁，
/// 请使用 [`IrqSafe`](crate::irq_safe::IrqSafe)。
pub struct Mutex<R: RawLock, T> {
    pub(crate) raw: R,
    pub(crate) data: UnsafeCell<T>,
}

// SAFETY: Mutex 通过 RawLock 的原子操作保证互斥访问。
// RawLock: Send + Sync（supertrait），T: Send 即可安全跨线程。
unsafe impl<R: RawLock, T: Send> Send for Mutex<R, T> {}
unsafe impl<R: RawLock, T: Send> Sync for Mutex<R, T> {}

/// `SpinLock<T>` 特化构造器——保持与原 API 完全兼容。
impl<T> Mutex<RawSpinLock, T> {
    #[must_use]
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            raw: RawSpinLock::new(name),
            data: UnsafeCell::new(data),
        }
    }
}

/// 泛型方法——适用于所有 `RawLock` 后端。
impl<R: RawLock, T> Mutex<R, T> {
    /// 获取锁，返回 RAII guard。
    ///
    /// # Panics
    /// 同一核心递归加锁（若底层 `RawLock` 支持检测）。
    pub fn lock(&self) -> MutexGuard<'_, R, T> {
        self.raw.check_recursive();
        self.raw.acquire();
        self.raw.set_owner();
        MutexGuard { mutex: self }
    }

    /// 尝试获取锁，不阻塞。
    pub fn try_lock(&self) -> Option<MutexGuard<'_, R, T>> {
        if self.raw.try_acquire() {
            self.raw.set_owner();
            Some(MutexGuard { mutex: self })
        } else {
            None
        }
    }

    /// 查询锁是否被持有。
    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    /// 锁名称（诊断用）。
    pub fn name(&self) -> &'static str {
        self.raw.name()
    }
}

/// RAII guard——丢弃时释放锁。
///
/// 所有锁后端共享同一个 guard 实现。
pub struct MutexGuard<'a, R: RawLock, T> {
    mutex: &'a Mutex<R, T>,
}

impl<R: RawLock, T> Deref for MutexGuard<'_, R, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &*self.mutex.data.get() }
    }
}

impl<R: RawLock, T> DerefMut for MutexGuard<'_, R, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<R: RawLock, T> Drop for MutexGuard<'_, R, T> {
    fn drop(&mut self) {
        self.mutex.raw.clear_owner();
        self.mutex.raw.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type SpinLock<T> = Mutex<RawSpinLock, T>;

    /// 基本的加锁/解锁流程
    #[test]
    fn lock_and_unlock() {
        let lock = SpinLock::new(42u32, "test");
        {
            let guard = lock.lock();
            assert_eq!(*guard, 42);
        }
        assert!(!lock.is_locked());
    }

    /// guard 提供可变访问
    #[test]
    fn guard_provides_mutable_access() {
        let lock = SpinLock::new(0u32, "test_mut");
        {
            let mut guard = lock.lock();
            *guard = 99;
        }
        let guard = lock.lock();
        assert_eq!(*guard, 99);
    }

    /// try_lock 在锁空闲时成功
    #[test]
    fn try_lock_succeeds_when_free() {
        let lock = SpinLock::new(7u32, "try_lock_test");
        let guard = lock.try_lock();
        assert!(guard.is_some());
        assert_eq!(*guard.expect("lock should succeed"), 7);
    }

    /// try_lock 在锁已持有时失败
    #[test]
    fn try_lock_fails_when_held() {
        let lock = SpinLock::new(0u32, "try_lock_held");
        let _g = lock.lock();
        let second = lock.try_lock();
        assert!(second.is_none());
    }

    /// guard 析构时自动释放锁
    #[test]
    fn guard_drop_releases_lock() {
        let lock = SpinLock::new(0u32, "drop_test");
        {
            let _g = lock.lock();
            assert!(lock.is_locked());
        }
        assert!(!lock.is_locked());
    }

    /// 多线程并发自增验证互斥正确性
    #[test]
    fn concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let lock = Arc::new(SpinLock::new(0u64, "concurrent"));
        let mut handles = Vec::new();

        for _ in 0..4 {
            let lock = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let mut g = lock.lock();
                    *g += 1;
                }
            }));
        }

        for h in handles {
            h.join().expect("线程应正常结束");
        }

        let g = lock.lock();
        assert_eq!(*g, 4000, "并发计数器最终值应为 4000");
    }

    /// 递归加锁应 panic
    #[test]
    #[should_panic(expected = "recursive lock")]
    fn recursive_lock_panics() {
        let lock = SpinLock::new(0u32, "recursive");
        let _g = lock.lock();
        let _g2 = lock.lock();
    }
}
