use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::interrupt_ops::HeldInterrupts;

/// 用于强制获取顺序的锁级别常量。
///
/// 数值更小的级别必须优先获取。
/// 在持有更高级别锁时获取更低级别的锁会触发 halt。
pub mod lock_level {
    pub const SCHED_LOCK: u8 = 0;
    pub const TASK_TABLE_LOCK: u8 = 1;
    pub const INTERRUPT_THREADS_LOCK: u8 = 2;
    pub const UNCLASSIFIED: u8 = 0xFF;
}

const NO_OWNER: usize = usize::MAX;

/// 原始自旋锁——基于 AtomicBool 的 TTAS（test-and-test-and-set）算法。
///
/// 不包含数据保护，仅提供互斥。由 `SpinLock` 和 `SpinLockIrq` 内部使用。
struct RawSpinLock {
    locked: AtomicBool,
    owner_core: AtomicUsize,
    name: &'static str,
}

impl RawSpinLock {
    const fn new(name: &'static str) -> Self {
        Self {
            locked: AtomicBool::new(false),
            owner_core: AtomicUsize::new(NO_OWNER),
            name,
        }
    }

    /// 自旋获取锁（TTAS 算法）。
    ///
    /// 外层用 Relaxed load 自旋（只读缓存行，不争总线），
    /// 内层用 compare_exchange_weak 尝试获取。
    #[inline]
    fn acquire(&self) {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
    }

    /// 尝试获取锁，失败返回 false。
    #[inline]
    fn try_acquire(&self) -> bool {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// 释放锁。
    #[inline]
    fn release(&self) {
        self.locked.store(false, Ordering::Release);
    }

    #[inline]
    fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }

    /// 递归加锁检测——同一核心再次获取同一把锁会死锁（自旋永不返回）。
    fn check_recursive(&self) {
        if self.owner_core.load(Ordering::Relaxed) == per_cpu::current_core_id() {
            Self::fatal(self.name, "recursive lock");
        }
    }

    fn set_owner(&self) {
        self.owner_core
            .store(per_cpu::current_core_id(), Ordering::Release);
    }

    fn clear_owner(&self) {
        self.owner_core.store(NO_OWNER, Ordering::Release);
    }

    #[cold]
    #[inline(never)]
    fn fatal(name: &str, reason: &str) -> ! {
        panic!("FATAL: SpinLock '{}': {}", name, reason);
    }
}

/// 自旋锁——不操作中断。
///
/// 适用于**不会在中断 handler 中获取**的锁。
/// 如果中断 handler 也会获取同一把锁，必须使用 [`SpinLockIrq`]。
///
/// # Usage
///
/// ```ignore
/// static MY_LOCK: SpinLock<MyData> = SpinLock::new(MyData::new(), "my_lock");
/// let guard = MY_LOCK.lock();
/// // guard 被丢弃时释放锁
/// ```
pub struct SpinLock<T> {
    raw: RawSpinLock,
    data: UnsafeCell<T>,
}

// SAFETY: SpinLock 内部使用原子操作保证互斥访问，T: Send 即可安全跨线程
unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    #[must_use]
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            raw: RawSpinLock::new(name),
            data: UnsafeCell::new(data),
        }
    }

    /// 获取锁，返回 RAII guard。
    ///
    /// # Panics
    /// 同一核心递归加锁。
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        self.raw.check_recursive();
        self.raw.acquire();
        self.raw.set_owner();
        SpinLockGuard { lock: self }
    }

    /// 尝试获取锁，不阻塞。
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        if self.raw.try_acquire() {
            self.raw.set_owner();
            Some(SpinLockGuard { lock: self })
        } else {
            None
        }
    }

    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }
}

/// RAII guard——丢弃时释放锁。
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.raw.clear_owner();
        self.lock.raw.release();
    }
}

/// 中断安全的自旋锁——获取时禁用中断，释放时恢复。
///
/// 适用于**中断 handler 也会获取**的锁（如调度锁、控制台锁）。
/// 支持锁级别顺序检查和 per-CPU 锁栈，防止死锁。
///
/// # Usage
///
/// ```ignore
/// static MY_LOCK: SpinLockIrq<MyData> = SpinLockIrq::new(MyData::new(), "my_lock");
/// let guard = MY_LOCK.lock();
/// // guard 被丢弃时恢复中断
/// ```
pub struct SpinLockIrq<T> {
    raw: RawSpinLock,
    data: UnsafeCell<T>,
    #[cfg_attr(not(target_os = "none"), allow(dead_code))]
    level: u8,
}

// SAFETY: 同 SpinLock
unsafe impl<T: Send> Send for SpinLockIrq<T> {}
unsafe impl<T: Send> Sync for SpinLockIrq<T> {}

impl<T> SpinLockIrq<T> {
    #[must_use]
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            raw: RawSpinLock::new(name),
            data: UnsafeCell::new(data),
            level: lock_level::UNCLASSIFIED,
        }
    }

    #[must_use]
    pub const fn new_with_level(data: T, name: &'static str, level: u8) -> Self {
        Self {
            raw: RawSpinLock::new(name),
            data: UnsafeCell::new(data),
            level,
        }
    }

    /// 获取锁，返回 RAII guard。
    ///
    /// 使用 [`HeldInterrupts`] 证明令牌管理中断状态：
    /// 获取时禁用中断，guard 析构时恢复。
    ///
    /// # Panics
    /// - 同一核心递归加锁
    /// - 锁级别顺序违反
    pub fn lock(&self) -> SpinLockIrqGuard<'_, T> {
        let held = HeldInterrupts::hold();

        self.raw.check_recursive();
        self.raw.acquire();
        self.post_acquire();

        SpinLockIrqGuard {
            lock: self,
            _held: held,
        }
    }

    /// 尝试获取锁，不阻塞。
    pub fn try_lock(&self) -> Option<SpinLockIrqGuard<'_, T>> {
        let held = HeldInterrupts::hold();

        if self.raw.try_acquire() {
            self.post_acquire();
            Some(SpinLockIrqGuard {
                lock: self,
                _held: held,
            })
        } else {
            // held 在此处 drop，自动恢复中断
            None
        }
    }

    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    /// 尝试获取裸锁（不操作中断、不检查锁级别、不压栈）——
    /// 用于任务窃取时在已持有自身调度锁的情况下获取另一核心的调度锁。
    ///
    /// # Safety
    ///
    /// 调用者必须保证：
    /// 1. 中断已被禁用
    /// 2. 成功后必须调用 `unlock_raw_no_irq()` 释放
    pub unsafe fn try_lock_raw_no_irq(&self) -> bool {
        if self.raw.try_acquire() {
            self.raw.set_owner();
            true
        } else {
            false
        }
    }

    /// 释放裸锁（不操作中断、不弹出锁栈）——与 `try_lock_raw_no_irq` 配对。
    ///
    /// # Safety
    ///
    /// 必须在持有锁的情况下调用。
    pub unsafe fn unlock_raw_no_irq(&self) {
        self.raw.clear_owner();
        self.raw.release();
    }

    fn post_acquire(&self) {
        self.raw.set_owner();
        #[cfg(target_os = "none")]
        {
            self.check_lock_order();
            self.push_lock_stack();
        }
    }

    fn pre_release(&self) {
        #[cfg(target_os = "none")]
        {
            self.pop_lock_stack();
        }
        self.raw.clear_owner();
    }

    #[cfg(target_os = "none")]
    fn check_lock_order(&self) {
        // SAFETY: 中断已禁用，无同核心并发访问
        let stack = unsafe { per_cpu::LOCK_STACK.get_mut() };
        if !stack.check_order(self.level, lock_level::UNCLASSIFIED) {
            RawSpinLock::fatal(self.raw.name, "lock order violation");
        }
    }

    #[cfg(target_os = "none")]
    fn push_lock_stack(&self) {
        // SAFETY: 中断已禁用
        let stack = unsafe { per_cpu::LOCK_STACK.get_mut() };
        stack.push(self as *const Self as *const (), self.level);
    }

    #[cfg(target_os = "none")]
    fn pop_lock_stack(&self) {
        // SAFETY: 中断已禁用
        let stack = unsafe { per_cpu::LOCK_STACK.get_mut() };
        stack.pop(self as *const Self as *const ());
    }
}

/// RAII guard（中断安全版）。
///
/// 自定义 `Drop` 先释放锁，然后 `_held` 字段由编译器自动析构，恢复中断。
/// 保证「先释放锁，后恢复中断」的正确顺序。
pub struct SpinLockIrqGuard<'a, T> {
    lock: &'a SpinLockIrq<T>,
    /// 中断禁用的证明令牌——在自定义 Drop 之后由编译器析构
    _held: HeldInterrupts,
}

impl<T> Deref for SpinLockIrqGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockIrqGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinLockIrqGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.pre_release();
        self.lock.raw.release();
        // _held 在此之后由编译器自动 drop，恢复中断状态
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// SpinLockIrq 基本加锁/解锁流程
    #[test]
    fn irq_lock_and_unlock() {
        let lock = SpinLockIrq::new(42u32, "irq_test");
        {
            let guard = lock.lock();
            assert_eq!(*guard, 42);
        }
        assert!(!lock.is_locked());
    }

    /// SpinLockIrq try_lock 成功
    #[test]
    fn irq_try_lock() {
        let lock = SpinLockIrq::new(0u32, "irq_try");
        assert!(lock.try_lock().is_some());
    }

    /// SpinLockIrq try_lock 在锁已持有时失败
    #[test]
    fn irq_try_lock_fails_when_held() {
        let lock = SpinLockIrq::new(0u32, "irq_try_held");
        let _g = lock.lock();
        assert!(lock.try_lock().is_none());
    }

    /// 带锁级别的 SpinLockIrq 构造
    #[test]
    fn irq_new_with_level() {
        let lock = SpinLockIrq::new_with_level(0u32, "leveled", lock_level::SCHED_LOCK);
        let _g = lock.lock();
        assert!(lock.is_locked());
    }

    /// SpinLockIrq 多线程并发访问
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

    /// 裸锁获取/释放配对
    #[test]
    fn try_lock_raw_no_irq_and_unlock() {
        let lock = SpinLockIrq::new((), "raw_no_irq_test");
        assert!(unsafe { lock.try_lock_raw_no_irq() });
        assert!(lock.is_locked());
        unsafe { lock.unlock_raw_no_irq() };
        assert!(!lock.is_locked());
    }

    /// 裸锁在锁已持有时获取失败
    #[test]
    fn try_lock_raw_no_irq_fails_when_held() {
        let lock = SpinLockIrq::new((), "raw_no_irq_fail");
        let _g = lock.lock();
        assert!(!unsafe { lock.try_lock_raw_no_irq() });
    }

    /// SpinLock 递归加锁应 panic
    #[test]
    #[should_panic(expected = "recursive lock")]
    fn recursive_lock_panics() {
        let lock = SpinLock::new(0u32, "recursive");
        let _g = lock.lock();
        let _g2 = lock.lock();
    }

    /// SpinLockIrq 递归加锁应 panic
    #[test]
    #[should_panic(expected = "recursive lock")]
    fn irq_recursive_lock_panics() {
        let lock = SpinLockIrq::new(0u32, "irq_recursive");
        let _g = lock.lock();
        let _g2 = lock.lock();
    }

    /// ```compile_fail
    /// use sync::HeldInterrupts;
    /// let a = sync::interrupt_ops::HeldInterrupts::hold();
    /// let b = a;
    /// drop(a); // use after move — 不应编译通过
    /// ```
    #[test]
    fn held_interrupts_is_not_copy() {
        // 运行时验证 size；编译期验证 !Copy 由上方 doc test 保证
        assert_eq!(core::mem::size_of::<HeldInterrupts>(), 1);
    }

    /// HeldInterrupts hold 保存状态、drop 恢复
    #[test]
    fn held_interrupts_hold_and_drop() {
        let held = HeldInterrupts::hold();
        assert!(!held.was_enabled());
        drop(held);
    }
}
