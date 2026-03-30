//! Layer 0: 原始锁机制——trait 抽象 + TTAS 自旋锁实现。
//!
//! `RawLock` trait 定义纯互斥协议，不涉及数据保护、中断管理或锁序检查。
//! 更换锁算法（TTAS → ticket → MCS）只需新增一个 `impl RawLock`。

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const NO_OWNER: usize = usize::MAX;

/// 原始锁机制 trait——只负责互斥。
///
/// # Safety
///
/// 实现者必须保证：
/// 1. `acquire` 返回后当前核心独占锁
/// 2. `release` 正确释放锁，使其他核心可获取
/// 3. 所有方法在多核并发调用下无 UB
pub unsafe trait RawLock: Send + Sync {
    /// 阻塞获取。
    fn acquire(&self);

    /// 非阻塞尝试，成功返回 `true`。
    fn try_acquire(&self) -> bool;

    /// 释放锁。
    fn release(&self);

    /// 查询锁是否被持有（仅供诊断，结果可能立即过期）。
    fn is_locked(&self) -> bool;

    /// 锁名称（诊断用）。
    fn name(&self) -> &'static str;

    /// 递归加锁检测——默认空实现。
    fn check_recursive(&self) {}

    /// 设置当前核心为 owner——默认空实现。
    fn set_owner(&self) {}

    /// 清除 owner——默认空实现。
    fn clear_owner(&self) {}
}

/// TTAS（test-and-test-and-set）自旋锁。
///
/// 外层用 `Relaxed` load 自旋（只读缓存行，不争总线），
/// 内层用 `compare_exchange_weak` 尝试获取。
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
