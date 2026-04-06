//! 泛型互斥锁——数据保护 + RAII guard + 抢占管理 + 锁序检查。

use core::cell::UnsafeCell;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use interrupt_state::PreemptGuard;

use crate::raw::{RawLock, RawSpinLock};

/// 泛型互斥锁——通过 `R: RawLock` 参数化底层锁算法。
///
/// 获取锁时自动禁用抢占、检查递归加锁、参与锁序检查。
/// 不涉及中断管理。如果中断 handler 也会获取同一把锁，
/// 请使用 [`IrqSafe`](crate::irq_safe::IrqSafe)。
pub struct Mutex<R: RawLock, T> {
    pub(crate) raw: R,
    pub(crate) data: UnsafeCell<T>,
    level: u8,
}

// SAFETY: Mutex 通过 RawLock 的原子操作保证互斥访问。
// RawLock: Send + Sync（supertrait），T: Send 即可安全跨线程。
unsafe impl<R: RawLock, T: Send> Send for Mutex<R, T> {}
unsafe impl<R: RawLock, T: Send> Sync for Mutex<R, T> {}

/// `SpinLock<T>` 特化构造器。
impl<T> Mutex<RawSpinLock, T> {
    /// 创建自旋锁。
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
}

/// 泛型方法——适用于所有 `RawLock` 后端。
impl<R: RawLock, T> Mutex<R, T> {
    /// 获取锁，返回 RAII guard。
    ///
    /// 流程：中断上下文检查 → 禁用抢占 → 递归检测 → 自旋获取 → 设置 owner → 压锁栈。
    ///
    /// # Panics
    /// - 在中断上下文中调用（裸机环境）——应使用 `SpinLockIrq`
    /// - 同一核心递归加锁（若底层 `RawLock` 支持检测）
    /// - 锁级别顺序违反（裸机环境）
    pub fn lock(&self) -> MutexGuard<'_, R, T> {
        assert!(
            !interrupt_state::is_in_interrupt(),
            "SpinLock '{}': 在中断上下文中调用，应使用 SpinLockIrq",
            self.raw.name()
        );

        let preempt = PreemptGuard::disable();
        self.raw.check_recursive();
        self.raw.acquire();
        self.raw.set_owner();
        self.push_lock_stack();
        MutexGuard {
            mutex: self,
            preempt,
            _not_send: PhantomData,
        }
    }

    /// 尝试获取锁，不阻塞。
    ///
    /// # Panics
    /// - 在中断上下文中调用（裸机环境）——应使用 `SpinLockIrq`
    /// - 锁级别顺序违反（裸机环境）
    pub fn try_lock(&self) -> Option<MutexGuard<'_, R, T>> {
        assert!(
            !interrupt_state::is_in_interrupt(),
            "SpinLock '{}': 在中断上下文中调用，应使用 SpinLockIrq",
            self.raw.name()
        );

        let preempt = PreemptGuard::disable();

        if self.raw.try_acquire() {
            self.raw.set_owner();
            self.push_lock_stack();
            Some(MutexGuard {
                mutex: self,
                preempt,
                _not_send: PhantomData,
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

    /// 查询锁是否被持有。
    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    /// 锁名称（诊断用）。
    pub fn name(&self) -> &'static str {
        self.raw.name()
    }

    /// 锁级别。
    pub fn level(&self) -> u8 {
        self.level
    }

    /// 获取锁后压入 per-CPU 锁栈——检查锁序是否合法。
    fn push_lock_stack(&self) {
        let _held = interrupt_state::HeldInterrupts::hold();
        // SAFETY: 中断已禁用，无同核心并发访问
        let stack = unsafe { crate::LOCK_STACK.get_mut() };
        if !stack.check_order(self.level) {
            panic!(
                "SpinLock '{}': lock order violation (level={})",
                self.raw.name(),
                self.level,
            );
        }
        stack.push(self as *const Self as *const (), self.level);
    }

    /// 释放锁前从 per-CPU 锁栈弹出。
    fn pop_lock_stack(&self) {
        let _held = interrupt_state::HeldInterrupts::hold();
        // SAFETY: 中断已禁用，无同核心并发访问
        let stack = unsafe { crate::LOCK_STACK.get_mut() };
        stack.pop(self as *const Self as *const ());
    }
}

impl<R: RawLock, T> fmt::Debug for Mutex<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutex")
            .field("name", &self.raw.name())
            .field("locked", &self.raw.is_locked())
            .field("level", &self.level)
            .finish()
    }
}

/// RAII guard——丢弃时释放锁并恢复抢占状态。
///
/// 所有锁后端共享同一个 guard 实现。
/// `!Send`——guard 必须在获取锁的同一核心上释放，
/// 防止任务迁移导致跨核释放。
///
/// 析构顺序：弹锁栈 → 清 owner → 释放锁 → 恢复抢占（`preempt` 自动 drop）。
pub struct MutexGuard<'a, R: RawLock, T> {
    mutex: &'a Mutex<R, T>,
    #[expect(
        dead_code,
        reason = "持有 PreemptGuard 以利用其 Drop 恢复抢占状态，不需要读取"
    )]
    preempt: PreemptGuard,
    /// `*mut ()` 是 `!Send`——使整个 guard 也变为 `!Send`。
    _not_send: PhantomData<*mut ()>,
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
        // 1. 弹出锁栈（在释放锁之前，确保锁栈状态一致）
        self.mutex.pop_lock_stack();
        // 2. 清除 owner
        self.mutex.raw.clear_owner();
        // 3. 释放锁
        self.mutex.raw.release();
        // 4. preempt guard 自动 drop——恢复抢占
        //    （Rust 按字段声明顺序逆序 drop，preempt 在 mutex 之后声明，
        //     但显式 Drop impl 中字段不会自动 drop，只有非 Drop 字段才会。
        //     这里 PreemptGuard 实现了 Drop，Rust 在我们的 drop() 返回后
        //     会自动 drop 所有字段，包括 self.preempt。）
    }
}

impl<R: RawLock, T: fmt::Debug> fmt::Debug for MutexGuard<'_, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
