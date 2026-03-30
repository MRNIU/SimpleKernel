//! Layer 1b: 中断安全锁——在 [`Mutex`] 之上叠加中断管理和锁序检查。
//!
//! `IrqSafe<R, T>` **组合**（而非复制）`Mutex`，附加：
//! - 获取时禁用中断（`HeldInterrupts` proof token）
//! - per-CPU 锁栈 + 级别检查（死锁预防）
//!
//! 所有 `Deref`/`DerefMut`/锁释放逻辑由内部 `MutexGuard` 提供，不重复实现。
//!
//! ## 设计决策
//!
//! ### 1. 自旋期间恢复中断（loop-try-reopen 模式）
//!
//! 参考 Theseus OS 的 [`DeadlockPrevention::lock()`][theseus-lock] 实现：
//! 当锁竞争失败时，**立即恢复中断**，在中断开启状态下自旋等待，
//! 仅在 CAS 尝试获取的瞬间关闭中断。
//!
//! 这避免了"无界等待 + 全程关中断"的组合：
//! - 等待期间 timer tick、IPI（如 TLB shootdown）可正常响应
//! - 消除"Core A 等 Core B 响应 IPI，Core B 关中断等 Core A 释放锁"的活锁风险
//!
//! ```text
//! ┌─ loop ──────────────────────────────────────────────┐
//! │  HeldInterrupts::hold()     ← 关中断               │
//! │  try_acquire()              ← CAS 尝试             │
//! │  ├─ 成功 → set_owner, post_acquire, return guard   │
//! │  └─ 失败 → drop(held)      ← 恢复中断             │
//! │            while is_locked() { spin_loop() }        │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! 对比 Linux：`spin_lock_irqsave` 全程关中断，但 Linux 使用 qspinlock（MCS）
//! 保证 O(1) 等待和 FIFO 公平性。TTAS 等待时间无上界，
//! 因此组合"无界等待 + 全程关中断"是危险的。
//!
//! [theseus-lock]: https://github.com/theseus-os/Theseus/blob/theseus_main/libs/sync/src/lib.rs
//!
//! ### 2. `ManuallyDrop` 显式析构顺序
//!
//! `IrqSafeGuard` 的析构必须严格遵守顺序：
//! 1. 弹出 per-CPU 锁栈
//! 2. 释放锁（clear_owner + release）
//! 3. 恢复中断
//!
//! 若依赖字段声明顺序（[RFC 1857] / [Reference §Destructors]），
//! 重构时交换字段会**静默破坏**此不变量——编译器不会报错，
//! 但中断在锁释放前恢复，中断 handler 看到锁仍被持有而死锁。
//!
//! 采用 `ManuallyDrop` + 显式 `drop()` 调用
//! （参考 Theseus OS [`MutexGuard`][theseus-guard]），
//! 将析构顺序从隐式布局依赖提升为显式代码控制。
//!
//! [RFC 1857]: https://rust-lang.github.io/rfcs/1857-stabilize-drop-order.html
//! [Reference §Destructors]: https://doc.rust-lang.org/reference/destructors.html
//! [theseus-guard]: https://github.com/theseus-os/Theseus/blob/theseus_main/libs/sync/src/mutex.rs
//!
//! ### 3. `try_lock` 快速路径（参考 Theseus `EXPENSIVE` 优化）
//!
//! 关中断是昂贵操作（保存/恢复 CPU 状态寄存器）。
//! 当锁明显被持有时，先用 `Relaxed` load 检查，直接返回 `None`，
//! 跳过无谓的中断状态切换。

use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};

use crate::mutex::{Mutex, MutexGuard};
use crate::raw::{RawLock, RawSpinLock};
use interrupt_state::HeldInterrupts;

/// 用于强制获取顺序的锁级别常量。
///
/// 数值更小的级别必须优先获取。
/// 在持有更高级别锁时获取更低级别的锁会触发 panic。
pub mod lock_level {
    pub const SCHED_LOCK: u8 = 0;
    pub const TASK_TABLE_LOCK: u8 = 1;
    pub const INTERRUPT_THREADS_LOCK: u8 = 2;
    pub const UNCLASSIFIED: u8 = 0xFF;
}

/// 中断安全锁——获取时禁用中断，释放时恢复。
///
/// 内部组合 [`Mutex<R, T>`]，在此之上叠加中断管理和锁序检查。
/// 适用于**中断 handler 也会获取**的锁（如调度锁、控制台锁）。
///
/// # Usage
///
/// ```ignore
/// static MY_LOCK: SpinLockIrq<MyData> = SpinLockIrq::new(MyData::new(), "my_lock");
/// let guard = MY_LOCK.lock();
/// // guard 被丢弃时恢复中断
/// ```
pub struct IrqSafe<R: RawLock, T> {
    mutex: Mutex<R, T>,
    #[cfg_attr(not(target_os = "none"), allow(dead_code))]
    level: u8,
}

// SAFETY: 安全性由内部 Mutex 保证。
unsafe impl<R: RawLock, T: Send> Send for IrqSafe<R, T> {}
unsafe impl<R: RawLock, T: Send> Sync for IrqSafe<R, T> {}

/// `SpinLockIrq<T>` 特化构造器——保持与原 API 完全兼容。
impl<T> IrqSafe<RawSpinLock, T> {
    #[must_use]
    pub const fn new(data: T, name: &'static str) -> Self {
        Self {
            mutex: Mutex::new(data, name),
            level: lock_level::UNCLASSIFIED,
        }
    }

    #[must_use]
    pub const fn new_with_level(data: T, name: &'static str, level: u8) -> Self {
        Self {
            mutex: Mutex::new(data, name),
            level,
        }
    }
}

/// 泛型方法——适用于所有 `RawLock` 后端。
impl<R: RawLock, T> IrqSafe<R, T> {
    /// 获取锁，返回 RAII guard。
    ///
    /// 采用 **loop-try-reopen** 模式（参考 Theseus OS）：
    /// 每次迭代仅在 CAS 尝试期间关中断，失败后立即恢复，
    /// 避免长时间关中断导致的中断延迟和潜在活锁。
    ///
    /// ```text
    /// loop {
    ///     关中断 → check_recursive → try_acquire
    ///     ├─ 成功 → 返回 guard（中断保持关闭）
    ///     └─ 失败 → 恢复中断 → 自旋等待（中断开启）
    /// }
    /// ```
    ///
    /// # Panics
    /// - 同一核心递归加锁
    /// - 锁级别顺序违反
    pub fn lock(&self) -> IrqSafeGuard<'_, R, T> {
        loop {
            let held = HeldInterrupts::hold();

            // 递归检测必须在关中断后执行——
            // 关中断保证 per-CPU owner_core 读取与当前核心一致
            self.mutex.raw.check_recursive();

            if let Some(inner) = self.mutex.try_lock() {
                self.post_acquire();
                return IrqSafeGuard {
                    inner: ManuallyDrop::new(inner),
                    irq_safe: self,
                    held: ManuallyDrop::new(held),
                };
            }

            // 获取失败——立即恢复中断，避免关中断下长时间自旋。
            // held 在此处 drop → 恢复中断状态。
            drop(held);

            // 中断开启状态下自旋等待——
            // timer tick、IPI 等可正常响应，不阻塞其他核心
            while self.mutex.raw.is_locked() {
                core::hint::spin_loop();
            }
        }
    }

    /// 尝试获取锁，不阻塞。
    ///
    /// 快速路径：锁已被持有时直接返回 `None`，
    /// 跳过中断状态切换的开销（参考 Theseus `EXPENSIVE` 优化）。
    pub fn try_lock(&self) -> Option<IrqSafeGuard<'_, R, T>> {
        // 快速路径：锁已被持有，避免无谓的中断状态切换。
        // Relaxed load 在此足够——即使读到过期值（false negative），
        // 后续 try_lock 的 CAS 会正确判断。
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
            // held 在此处 drop，自动恢复中断
            None
        }
    }

    /// 嵌套获取——调用者已关中断，用 proof token 证明。
    ///
    /// **有意跳过**中断管理和锁序检查，保留 RAII 自动释放。
    /// 典型场景：任务窃取时在已持有自身调度锁（level 0）的情况下，
    /// `try_lock` 获取另一核心的同级别调度锁——`try_lock` 语义保证不会死锁
    /// （失败立即返回 `None`），因此同级别锁获取是安全的。
    ///
    /// 替代原 `try_lock_raw_no_irq` / `unlock_raw_no_irq` 逃生舱，
    /// 将「中断已禁用」前置条件从 unsafe 注释升级为编译期类型约束。
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
            if !stack.check_order(self.level, lock_level::UNCLASSIFIED) {
                panic!(
                    "FATAL: SpinLock '{}': lock order violation",
                    self.mutex.name()
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

/// RAII guard（中断安全版）。
///
/// 使用 [`ManuallyDrop`] 显式控制析构顺序（参考 Theseus OS `MutexGuard`），
/// 避免依赖字段声明顺序（RFC 1857）——重构时交换字段不会静默破坏安全不变量。
///
/// 析构顺序：
/// 1. 弹出 per-CPU 锁栈（中断仍禁用）
/// 2. 释放锁（`MutexGuard::drop` → clear_owner + release）
/// 3. 恢复中断（`HeldInterrupts::drop` → 若之前开启则重新启用）
pub struct IrqSafeGuard<'a, R: RawLock, T> {
    /// 内部互斥 guard——`ManuallyDrop` 确保在自定义 `Drop` 中显式释放。
    inner: ManuallyDrop<MutexGuard<'a, R, T>>,
    #[cfg_attr(not(target_os = "none"), allow(dead_code))]
    irq_safe: &'a IrqSafe<R, T>,
    /// 中断禁用的证明令牌——`ManuallyDrop` 确保在锁释放**之后**才恢复中断。
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
        // ── 显式析构顺序（不依赖字段声明顺序）──────────────
        //
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
        let lock = SpinLockIrq::new_with_level(0u32, "leveled", lock_level::SCHED_LOCK);
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
