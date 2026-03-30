//! TTAS（test-and-test-and-set）自旋锁实现。

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::RawLock;

const NO_OWNER: usize = usize::MAX;

/// TTAS（test-and-test-and-set）自旋锁。
///
/// 外层用 `compare_exchange_weak` 尝试获取，
/// 失败后进入内层 `Relaxed` load 自旋（只读缓存行，不争总线），
/// 待锁释放后重新尝试 CAS。
/// 附带 owner 追踪，用于同核心递归加锁检测。
pub struct RawSpinLock {
    locked: AtomicBool,
    owner_core: AtomicUsize,
    name: &'static str,
}

impl RawSpinLock {
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self {
            locked: AtomicBool::new(false),
            owner_core: AtomicUsize::new(NO_OWNER),
            name,
        }
    }

    #[cold]
    #[inline(never)]
    fn fatal(name: &str, reason: &str) -> ! {
        panic!("FATAL: SpinLock '{}': {}", name, reason);
    }
}

// SAFETY: TTAS 算法通过 Acquire/Release 原子操作保证互斥。
unsafe impl RawLock for RawSpinLock {
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

    #[inline]
    fn try_acquire(&self) -> bool {
        self.locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    fn release(&self) {
        self.locked.store(false, Ordering::Release);
    }

    #[inline]
    fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }

    #[inline]
    fn name(&self) -> &'static str {
        self.name
    }

    /// 递归加锁检测——同一核心再次获取同一把锁会死锁（自旋永不返回）。
    fn check_recursive(&self) {
        if self.owner_core.load(Ordering::Relaxed) == per_cpu::current_core_id() {
            Self::fatal(self.name, "recursive lock");
        }
    }

    fn set_owner(&self) {
        self.owner_core
            .store(per_cpu::current_core_id(), Ordering::Relaxed);
    }

    fn clear_owner(&self) {
        self.owner_core.store(NO_OWNER, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 新建锁处于未锁定状态
    #[test]
    fn new_lock_is_unlocked() {
        let lock = RawSpinLock::new("test");
        assert!(!lock.is_locked());
    }

    /// acquire 后 is_locked 返回 true
    #[test]
    fn acquire_sets_locked() {
        let lock = RawSpinLock::new("test");
        lock.acquire();
        assert!(lock.is_locked());
        lock.release();
    }

    /// release 后 is_locked 返回 false
    #[test]
    fn release_clears_locked() {
        let lock = RawSpinLock::new("test");
        lock.acquire();
        lock.release();
        assert!(!lock.is_locked());
    }

    /// try_acquire 在锁空闲时成功
    #[test]
    fn try_acquire_succeeds_when_free() {
        let lock = RawSpinLock::new("test");
        assert!(lock.try_acquire());
        assert!(lock.is_locked());
        lock.release();
    }

    /// try_acquire 在锁已持有时失败
    #[test]
    fn try_acquire_fails_when_held() {
        let lock = RawSpinLock::new("test");
        lock.acquire();
        assert!(!lock.try_acquire());
        lock.release();
    }

    /// name 返回构造时的名称
    #[test]
    fn name_returns_given_name() {
        let lock = RawSpinLock::new("my_lock");
        assert_eq!(lock.name(), "my_lock");
    }

    /// set_owner/clear_owner 配合 check_recursive 检测递归加锁
    #[test]
    #[should_panic(expected = "recursive lock")]
    fn check_recursive_panics_when_same_core() {
        let lock = RawSpinLock::new("recursive");
        lock.set_owner();
        lock.check_recursive();
    }

    /// clear_owner 后 check_recursive 不再 panic
    #[test]
    fn check_recursive_ok_after_clear_owner() {
        let lock = RawSpinLock::new("cleared");
        lock.set_owner();
        lock.clear_owner();
        lock.check_recursive();
    }

    /// 多线程并发 acquire/release 验证互斥正确性
    #[test]
    fn concurrent_acquire_release() {
        use std::sync::Arc;
        use std::thread;

        let lock = Arc::new(RawSpinLock::new("concurrent"));
        let counter = Arc::new(core::sync::atomic::AtomicU64::new(0));
        let mut handles = Vec::new();

        for _ in 0..4 {
            let lock = Arc::clone(&lock);
            let counter = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    lock.acquire();
                    // 非原子 read-modify-write，若互斥失败会出现数据竞争
                    let val = counter.load(Ordering::Relaxed);
                    counter.store(val + 1, Ordering::Relaxed);
                    lock.release();
                }
            }));
        }

        for h in handles {
            h.join().expect("线程应正常结束");
        }

        assert_eq!(counter.load(Ordering::Relaxed), 4000);
    }
}
