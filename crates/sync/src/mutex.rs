// Copyright The SimpleKernel Contributors

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

// SAFETY: `Mutex` 不暴露未同步的 `T` 访问；移动锁本身不会移动已借出的 guard。
// `RawLock: Send + Sync` 负责跨核心原子同步，`T: Send` 保证受保护数据可在线程间转移。
unsafe impl<R: RawLock, T: Send> Send for Mutex<R, T> {}

// SAFETY: 所有共享访问都必须先取得 guard；guard 生命周期绑定到 `&self`，
// `UnsafeCell<T>` 只在持锁期间生成引用，因此 `T: Send` 足以允许 `&Mutex` 跨核心共享。
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
        // SAFETY: `HeldInterrupts` 已关闭当前核心中断，CPU-local `LOCK_STACK`
        // 不会被同核心中断重入并发修改；其他核心访问的是各自的 CPU-local 实例。
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
        // SAFETY: `HeldInterrupts` 已关闭当前核心中断，当前 guard 仍持有锁，
        // 因此弹栈与后续释放锁之间不会被同核心中断打断。
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
        // SAFETY: guard 只能由成功获取锁构造；锁未释放前不会再产生可变访问，
        // 返回引用的生命周期受 guard 约束。
        unsafe { &*self.mutex.data.get() }
    }
}

impl<R: RawLock, T> DerefMut for MutexGuard<'_, R, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: `&mut self` 保证同一个 guard 不会同时借出多个可变引用；
        // RawLock 互斥保证其他核心此时不能访问 `data`。
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<R: RawLock, T> Drop for MutexGuard<'_, R, T> {
    fn drop(&mut self) {
        // 释放锁前先弹栈，确保锁序诊断仍能看到当前锁处于持有状态。
        self.mutex.pop_lock_stack();

        // owner 只用于递归检测和诊断，必须在锁位释放前清除。
        self.mutex.raw.clear_owner();

        // Release store 让临界区内写入对下一位持锁者可见。
        self.mutex.raw.release();
        // `PreemptGuard` 在本函数返回后自动 drop，最后恢复抢占，避免持锁期间迁核。
    }
}

impl<R: RawLock, T: fmt::Debug> fmt::Debug for MutexGuard<'_, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
