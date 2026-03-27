use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::per_cpu;
#[cfg(not(test))]
use crate::sync::lock_stack::LockStackEntry;

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

use super::interrupt_ops;

const NO_OWNER: usize = usize::MAX;

/// 中断安全的自旋锁，支持锁级别顺序检查。
///
/// 基于 `spin::Mutex` 实现自旋逻辑，额外提供：
/// - 通过 RAII guard 自动禁用/恢复中断
/// - 递归加锁检测
/// - 锁级别层次强制，防止死锁
///
/// # Usage
/// ```ignore
/// static MY_LOCK: SpinLock<MyData> = SpinLock::new(MyData::new(), "my_lock");
/// let guard = MY_LOCK.lock();
/// // guard 被丢弃时恢复中断
/// ```
pub struct SpinLock<T> {
    inner: spin::Mutex<T>,
    owner_core: AtomicUsize,
    level: u8,
    name: &'static str,
}

unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    #[must_use]
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            inner: spin::Mutex::new(data),
            owner_core: AtomicUsize::new(NO_OWNER),
            level: lock_level::UNCLASSIFIED,
            name,
        }
    }

    #[must_use]
    pub const fn new_with_level(data: T, name: &'static str, level: u8) -> Self {
        Self {
            inner: spin::Mutex::new(data),
            owner_core: AtomicUsize::new(NO_OWNER),
            level,
            name,
        }
    }

    /// 获取锁，返回 RAII guard。
    ///
    /// # Panics
    /// - 同一核心递归加锁
    /// - 锁级别顺序违反
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        let saved_intr = interrupt_ops::get_status();
        interrupt_ops::disable();

        // 中断已关闭，同核心递归加锁会导致永久自旋，必须提前检测
        if self.owner_core.load(Ordering::Relaxed) == per_cpu::current_core_id() {
            Self::fatal(self.name, "recursive lock");
        }

        let guard = self.inner.lock();
        self.post_acquire();

        SpinLockGuard {
            lock: self,
            guard: ManuallyDrop::new(guard),
            saved_intr,
        }
    }

    /// 尝试在不阻塞的情况下获取锁。
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        let saved_intr = interrupt_ops::get_status();
        interrupt_ops::disable();

        match self.inner.try_lock() {
            Some(guard) => {
                self.post_acquire();
                Some(SpinLockGuard {
                    lock: self,
                    guard: ManuallyDrop::new(guard),
                    saved_intr,
                })
            }
            None => {
                if saved_intr {
                    // SAFETY: 恢复 try_lock 前的中断状态
                    unsafe { interrupt_ops::enable() };
                }
                None
            }
        }
    }

    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }

    /// 获取裸锁（不使用 RAII guard）——用于上下文切换时的 lock handoff 协议。
    ///
    /// 调用方必须在适当时机手动调用 `unlock_raw()` 释放锁。
    /// 此方法禁用中断，与 `lock()` 行为一致。
    ///
    /// # Safety
    /// 调用方必须保证：
    /// 1. 每次 `lock_raw()` 都有对应的 `unlock_raw()` 调用
    /// 2. `unlock_raw()` 在正确的核心上调用（可以是不同任务上下文，
    ///    但必须是同一物理核心）
    pub unsafe fn lock_raw(&self) {
        interrupt_ops::disable();

        if self.owner_core.load(Ordering::Relaxed) == per_cpu::current_core_id() {
            Self::fatal(self.name, "recursive lock (raw)");
        }

        // 自旋获取锁——spin::Mutex::lock() 返回 guard，
        // 我们立即 forget 它以避免 RAII 释放
        let guard = self.inner.lock();
        core::mem::forget(guard);

        self.post_acquire();
    }

    /// 释放裸锁（与 `lock_raw()` 配对使用）。
    ///
    /// # Safety
    /// 必须在持有锁的情况下调用，且与 `lock_raw()` 配对。
    pub unsafe fn unlock_raw(&self) {
        self.pre_release();

        // SAFETY: 调用方保证锁处于已获取状态
        unsafe { self.inner.force_unlock() };

        // SAFETY: 恢复 lock_raw 前的中断状态
        unsafe { interrupt_ops::enable() };
    }

    /// 尝试获取裸锁（不操作中断、不检查锁级别、不压栈）——
    /// 用于任务窃取时在已持有自身调度锁的情况下获取另一核心的调度锁。
    ///
    /// 成功返回 true，失败返回 false。
    ///
    /// # Safety
    ///
    /// 调用者必须保证：
    /// 1. 中断已被禁用（通常因为已通过 `lock_raw` 持有另一把锁）
    /// 2. 成功后必须调用 `unlock_raw_no_irq()` 释放
    /// 3. 不会导致死锁（使用 try 语义，失败不阻塞）
    pub unsafe fn try_lock_raw_no_irq(&self) -> bool {
        match self.inner.try_lock() {
            Some(guard) => {
                core::mem::forget(guard);
                self.owner_core
                    .store(per_cpu::current_core_id(), Ordering::Release);
                true
            }
            None => false,
        }
    }

    /// 释放裸锁（不操作中断、不弹出锁栈）——与 `try_lock_raw_no_irq` 配对。
    ///
    /// # Safety
    ///
    /// 必须在持有锁的情况下调用，且与 `try_lock_raw_no_irq` 配对。
    pub unsafe fn unlock_raw_no_irq(&self) {
        self.owner_core.store(NO_OWNER, Ordering::Release);
        // SAFETY: 调用方保证锁处于已获取状态
        unsafe { self.inner.force_unlock() };
    }

    fn post_acquire(&self) {
        self.owner_core
            .store(per_cpu::current_core_id(), Ordering::Release);
        // 锁级别检查和锁栈依赖 per-CPU 数据（固定大小数组），
        // 在宿主多线程测试中线程 ID 可能超出数组范围，因此仅在内核模式启用。
        #[cfg(not(test))]
        {
            self.check_lock_order();
            self.push_lock_stack();
        }
    }

    fn pre_release(&self) {
        #[cfg(not(test))]
        {
            self.pop_lock_stack();
        }
        self.owner_core.store(NO_OWNER, Ordering::Release);
    }

    #[cold]
    #[inline(never)]
    fn fatal(name: &str, reason: &str) -> ! {
        crate::logging::raw_put("FATAL: SpinLock '");
        crate::logging::raw_put(name);
        crate::logging::raw_put("': ");
        crate::util::halt::halt(reason);
    }

    #[cfg(not(test))]
    fn check_lock_order(&self) {
        if self.level == lock_level::UNCLASSIFIED {
            return;
        }
        // SAFETY: 中断已禁用，无同核心并发访问
        let stack = &unsafe { per_cpu::current_per_cpu() }.lock_stack;
        if stack.depth > 0 {
            let top = stack.entries[stack.depth - 1].level;
            if top != lock_level::UNCLASSIFIED && self.level <= top {
                Self::fatal(self.name, "lock order violation");
            }
        }
    }

    #[cfg(not(test))]
    fn push_lock_stack(&self) {
        // SAFETY: 中断已禁用
        let stack = &mut unsafe { per_cpu::current_per_cpu() }.lock_stack;
        if stack.depth >= crate::sync::lock_stack::LockStack::MAX_DEPTH {
            panic!(
                "SpinLock '{}': lock stack overflow (depth={})",
                self.name, stack.depth
            );
        }
        stack.entries[stack.depth] = LockStackEntry {
            lock_ptr: self as *const Self as *const (),
            level: self.level,
        };
        stack.depth += 1;
    }

    #[cfg(not(test))]
    fn pop_lock_stack(&self) {
        // SAFETY: 中断已禁用
        let stack = &mut unsafe { per_cpu::current_per_cpu() }.lock_stack;
        if stack.depth == 0 {
            panic!("SpinLock '{}': lock stack underflow", self.name);
        }
        if stack.entries[stack.depth - 1].lock_ptr != (self as *const Self as *const ()) {
            panic!(
                "SpinLock '{}': lock stack corrupted — 释放顺序与获取顺序不一致",
                self.name
            );
        }
        stack.depth -= 1;
    }
}

/// RAII guard。丢弃时释放锁并恢复中断状态。
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    guard: ManuallyDrop<spin::MutexGuard<'a, T>>,
    saved_intr: bool,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.pre_release();
        // SAFETY: guard 有效且仅在此处 drop 一次
        unsafe { ManuallyDrop::drop(&mut self.guard) };
        if self.saved_intr {
            // SAFETY: 恢复获取锁前的中断状态
            unsafe { interrupt_ops::enable() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_and_unlock() {
        let lock = SpinLock::new(42u32, "test");
        {
            let guard = lock.lock();
            assert_eq!(*guard, 42);
        }
        assert!(!lock.is_locked());
    }

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

    #[test]
    fn try_lock_succeeds_when_free() {
        let lock = SpinLock::new(7u32, "try_lock_test");
        let guard = lock.try_lock();
        assert!(guard.is_some());
        assert_eq!(*guard.expect("lock should succeed"), 7);
    }

    #[test]
    fn try_lock_fails_when_held() {
        let lock = SpinLock::new(0u32, "try_lock_held");
        let _g = lock.lock();
        let second = lock.try_lock();
        assert!(second.is_none());
    }

    #[test]
    fn guard_drop_releases_lock() {
        let lock = SpinLock::new(0u32, "drop_test");
        {
            let _g = lock.lock();
            assert!(lock.is_locked());
        }
        assert!(!lock.is_locked());
    }

    #[test]
    fn new_with_level() {
        let lock = SpinLock::new_with_level(0u32, "leveled", lock_level::SCHED_LOCK);
        let _g = lock.lock();
        assert!(lock.is_locked());
    }

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

    #[test]
    fn concurrent_guard_drop_releases() {
        use std::sync::Arc;
        use std::thread;

        // 验证多线程环境下 guard drop 正确释放锁
        let lock = Arc::new(SpinLock::new(Vec::<usize>::new(), "drop_concurrent"));
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
}
