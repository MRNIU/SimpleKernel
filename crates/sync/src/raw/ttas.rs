// Copyright The SimpleKernel Contributors

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

    /// 返回当前执行上下文的唯一标识——裸机核心 ID。
    #[inline]
    fn caller_id() -> usize {
        per_cpu::current_core_id()
    }
}

// SAFETY: TTAS 算法通过 Acquire/Release 原子操作保证互斥。
unsafe impl RawLock for RawSpinLock {
    #[inline]
    fn acquire(&self) {
        #[cfg(feature = "spin-timeout")]
        let mut spin_count: u64 = 0;

        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();

                #[cfg(feature = "spin-timeout")]
                {
                    spin_count += 1;
                    if spin_count >= config::SPINLOCK_TIMEOUT {
                        panic!(
                            "SpinLock '{}': spin timeout after {} iterations \
                             (owner_core={}, current_core={})",
                            self.name,
                            spin_count,
                            self.owner_core.load(Ordering::Relaxed),
                            Self::caller_id(),
                        );
                    }
                }
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
        if self.owner_core.load(Ordering::Relaxed) == Self::caller_id() {
            Self::fatal(self.name, "recursive lock");
        }
    }

    fn set_owner(&self) {
        self.owner_core.store(Self::caller_id(), Ordering::Relaxed);
    }

    fn clear_owner(&self) {
        self.owner_core.store(NO_OWNER, Ordering::Relaxed);
    }
}
