// Copyright The SimpleKernel Contributors

//! 内核同步原语——分层自旋锁、中断安全锁和锁序检查。
//!
//! 架构设计详见 `crates/sync/AGENTS.md`。

#![no_std]

pub(crate) mod irq_safe;
pub mod lock_stack;
pub(crate) mod mutex;
pub(crate) mod raw;

use per_cpu::cpu_local;

/// Per-CPU 锁栈——强制锁获取顺序，防止死锁。
#[cpu_local]
pub static LOCK_STACK: lock_stack::LockStack = lock_stack::LockStack::new();

/// 自旋锁——不操作中断。
pub type SpinLock<T> = mutex::Mutex<raw::RawSpinLock, T>;

/// 自旋锁 RAII guard。
pub type SpinLockGuard<'a, T> = mutex::MutexGuard<'a, raw::RawSpinLock, T>;

/// 中断安全的自旋锁。
pub type SpinLockIrq<T> = irq_safe::IrqSafe<raw::RawSpinLock, T>;

/// 中断安全的自旋锁 RAII guard。
pub type SpinLockIrqGuard<'a, T> = irq_safe::IrqSafeGuard<'a, raw::RawSpinLock, T>;

pub use interrupt_state::HeldInterrupts;
pub use lock_stack::lock_level;
