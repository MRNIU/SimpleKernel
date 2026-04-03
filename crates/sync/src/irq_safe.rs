//! 中断安全锁——直接组合 `RawLock` + `UnsafeCell`，叠加中断管理和锁序检查。
//!
//! 不经过 [`Mutex`]，避免双重锁栈推入和不必要的 `PreemptGuard` 开销
//! （中断禁用 ⊃ 抢占禁用）。
//!
//! 设计决策详见 `crates/sync/README.md`。

use core::cell::UnsafeCell;
use core::fmt;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};

use crate::lock_stack::lock_level;
use crate::raw::{RawLock, RawSpinLock};
use interrupt_state::HeldInterrupts;

/// 中断安全锁——获取时禁用中断，释放时恢复。
///
/// 直接组合 `R: RawLock` + `UnsafeCell<T>`，自行管理锁栈，
/// 不依赖 [`Mutex`](crate::mutex::Mutex)。
pub struct IrqSafe<R: RawLock, T> {
    raw: R,
    data: UnsafeCell<T>,
    level: u8,
}

// SAFETY: IrqSafe 通过 RawLock 的原子操作 + 中断禁用保证互斥访问。
// T: Send 即可安全跨线程——锁保证同一时刻只有一个核心访问 T。
unsafe impl<R: RawLock, T: Send> Send for IrqSafe<R, T> {}
unsafe impl<R: RawLock, T: Send> Sync for IrqSafe<R, T> {}

impl<T> IrqSafe<RawSpinLock, T> {
    /// 创建中断安全锁。
    ///
    /// # Arguments
    /// - `data` — 被保护的数据
    /// - `name` — 锁名称（诊断用）
    /// - `level` — 锁级别（用于锁序检查，参见 [`lock_level`]）
    #[must_use]
    pub const fn new(data: T, name: &'static str, level: u8) -> Self {
        Self {
            raw: RawSpinLock::new(name),
            data: UnsafeCell::new(data),
            level,
        }
    }

    /// 创建不参与锁序检查的中断安全锁——便捷构造器。
    ///
    /// 等价于 `new(data, name, lock_level::UNSPECIFIED)`。
    #[must_use]
    pub const fn new_unordered(data: T, name: &'static str) -> Self {
        Self::new(data, name, lock_level::UNSPECIFIED)
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
            self.raw.check_recursive();

            if self.raw.try_acquire() {
                self.raw.set_owner();
                self.post_acquire();
                return IrqSafeGuard {
                    irq_safe: self,
                    held: ManuallyDrop::new(held),
                };
            }

            drop(held);

            while self.raw.is_locked() {
                core::hint::spin_loop();
            }
        }
    }

    /// 尝试获取锁，不阻塞。
    pub fn try_lock(&self) -> Option<IrqSafeGuard<'_, R, T>> {
        if self.raw.is_locked() {
            return None;
        }

        let held = HeldInterrupts::hold();

        if self.raw.try_acquire() {
            self.raw.set_owner();
            self.post_acquire();
            Some(IrqSafeGuard {
                irq_safe: self,
                held: ManuallyDrop::new(held),
            })
        } else {
            None
        }
    }

    /// 闭包 API——获取锁、执行闭包、自动释放。
    ///
    /// 比手动持有 guard 更清晰地界定临界区边界。
    ///
    /// # Panics
    /// 与 [`lock()`](Self::lock) 相同。
    pub fn with<Ret>(&self, f: impl FnOnce(&mut T) -> Ret) -> Ret {
        let mut guard = self.lock();
        f(&mut *guard)
    }

    /// 嵌套获取——调用者已关中断，用 proof token 证明。
    ///
    /// 跳过中断管理和锁序检查，保留 RAII 自动释放。
    /// 典型场景：任务窃取时获取另一核心的同级别调度锁。
    pub fn try_lock_nested(&self, _proof: &HeldInterrupts) -> Option<IrqSafeNestedGuard<'_, R, T>> {
        if self.raw.try_acquire() {
            self.raw.set_owner();
            Some(IrqSafeNestedGuard { irq_safe: self })
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

    /// 获取锁后压入 per-CPU 锁栈——检查锁序是否合法。
    ///
    /// 仅在裸机环境（`target_os = "none"`）生效；宿主机测试跳过
    /// （per-CPU 锁栈在宿主机上所有线程共享同一实例，多线程测试会产生竞态）。
    fn post_acquire(&self) {
        #[cfg(target_os = "none")]
        {
            // SAFETY: 中断已禁用，无同核心并发访问
            let stack = unsafe { crate::LOCK_STACK.get_mut() };
            if !stack.check_order(self.level) {
                panic!(
                    "SpinLockIrq '{}': lock order violation (level={})",
                    self.raw.name(),
                    self.level,
                );
            }
            stack.push(self as *const Self as *const (), self.level);
        }
    }

    /// 释放锁前从 per-CPU 锁栈弹出。
    fn pop_lock_stack(&self) {
        #[cfg(target_os = "none")]
        {
            // SAFETY: 中断已禁用，无同核心并发访问
            let stack = unsafe { crate::LOCK_STACK.get_mut() };
            stack.pop(self as *const Self as *const ());
        }
    }
}

impl<R: RawLock, T> fmt::Debug for IrqSafe<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IrqSafe")
            .field("name", &self.raw.name())
            .field("locked", &self.raw.is_locked())
            .field("level", &self.level)
            .finish()
    }
}

/// RAII guard（中断安全版）——析构顺序由 `ManuallyDrop` 显式控制。
///
/// 析构顺序：弹锁栈 → 清 owner → 释放锁 → 恢复中断。
pub struct IrqSafeGuard<'a, R: RawLock, T> {
    irq_safe: &'a IrqSafe<R, T>,
    held: ManuallyDrop<HeldInterrupts>,
}

impl<R: RawLock, T> Deref for IrqSafeGuard<'_, R, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &*self.irq_safe.data.get() }
    }
}

impl<R: RawLock, T> DerefMut for IrqSafeGuard<'_, R, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &mut *self.irq_safe.data.get() }
    }
}

impl<R: RawLock, T> Drop for IrqSafeGuard<'_, R, T> {
    fn drop(&mut self) {
        // 1. 弹出锁栈（中断仍禁用，per-CPU 访问安全）
        self.irq_safe.pop_lock_stack();

        // 2. 清除 owner
        self.irq_safe.raw.clear_owner();

        // 3. 释放锁
        self.irq_safe.raw.release();

        // 4. 恢复中断（若获取前中断已启用）
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
/// 不参与锁栈管理（嵌套获取跳过锁序检查）。
pub struct IrqSafeNestedGuard<'a, R: RawLock, T> {
    irq_safe: &'a IrqSafe<R, T>,
}

impl<R: RawLock, T> Deref for IrqSafeNestedGuard<'_, R, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &*self.irq_safe.data.get() }
    }
}

impl<R: RawLock, T> DerefMut for IrqSafeNestedGuard<'_, R, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &mut *self.irq_safe.data.get() }
    }
}

impl<R: RawLock, T> Drop for IrqSafeNestedGuard<'_, R, T> {
    fn drop(&mut self) {
        // 清除 owner → 释放锁（不操作锁栈——嵌套获取时未压栈）
        self.irq_safe.raw.clear_owner();
        self.irq_safe.raw.release();
    }
}

impl<R: RawLock, T: fmt::Debug> fmt::Debug for IrqSafeNestedGuard<'_, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type SpinLockIrq<T> = IrqSafe<RawSpinLock, T>;

    /// 基本加锁/解锁流程
    #[test]
    fn irq_lock_and_unlock() {
        let lock = SpinLockIrq::new(42u32, "irq_test", lock_level::CONSOLE);
        {
            let guard = lock.lock();
            assert_eq!(*guard, 42);
        }
        assert!(!lock.is_locked());
    }

    /// try_lock 成功
    #[test]
    fn irq_try_lock() {
        let lock = SpinLockIrq::new(0u32, "irq_try", lock_level::CONSOLE);
        assert!(lock.try_lock().is_some());
    }

    /// try_lock 在锁已持有时失败
    #[test]
    fn irq_try_lock_fails_when_held() {
        let lock = SpinLockIrq::new(0u32, "irq_try_held", lock_level::CONSOLE);
        let _g = lock.lock();
        assert!(lock.try_lock().is_none());
    }

    /// 带指定锁级别的构造
    #[test]
    fn irq_new_with_level() {
        let lock = SpinLockIrq::new(0u32, "leveled", lock_level::SCHED);
        let _g = lock.lock();
        assert!(lock.is_locked());
    }

    /// new_unordered 便捷构造器
    #[test]
    fn irq_new_unordered() {
        let lock = SpinLockIrq::new_unordered(99u32, "unordered");
        let guard = lock.lock();
        assert_eq!(*guard, 99);
    }

    /// with 闭包 API——获取锁、执行闭包、自动释放
    #[test]
    fn irq_with_closure() {
        let lock = SpinLockIrq::new(10u32, "with_test", lock_level::CONSOLE);
        let result = lock.with(|val| {
            *val += 5;
            *val
        });
        assert_eq!(result, 15);
        assert!(!lock.is_locked());

        // 验证数据确实被修改
        let guard = lock.lock();
        assert_eq!(*guard, 15);
    }

    /// 多线程并发访问
    #[test]
    fn irq_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let lock = Arc::new(SpinLockIrq::new(
            Vec::<usize>::new(),
            "irq_concurrent",
            lock_level::CONSOLE,
        ));
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
        let lock = SpinLockIrq::new((), "nested_test", lock_level::CONSOLE);
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
        let lock = SpinLockIrq::new((), "nested_fail", lock_level::CONSOLE);
        let _g = lock.lock();
        let held = HeldInterrupts::hold();
        assert!(lock.try_lock_nested(&held).is_none());
    }

    /// 递归加锁应 panic
    #[test]
    #[should_panic(expected = "recursive lock")]
    fn irq_recursive_lock_panics() {
        let lock = SpinLockIrq::new(0u32, "irq_recursive", lock_level::CONSOLE);
        let _g = lock.lock();
        let _g2 = lock.lock();
    }
}
