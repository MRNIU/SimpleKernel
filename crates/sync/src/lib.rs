#![cfg_attr(not(test), no_std)]

//! 内核同步原语——分层架构。
//!
//! ```text
//! Layer 2: 类型别名（用户接触的具体类型）
//!   SpinLock<T>    = Mutex<RawSpinLock, T>
//!   SpinLockIrq<T> = IrqSafe<RawSpinLock, T>
//!
//! Layer 1: 组合层（两个独立维度）
//!   Mutex<R, T>        — 数据保护 + RAII guard
//!   IrqSafe<R, T>      — Mutex + 关中断 + 锁序检查
//!
//! Layer 0: 原始锁机制（trait 抽象）
//!   trait RawLock       — acquire / try_acquire / …
//!   RawSpinLock         — TTAS 实现
//! ```
//!
//! 中断状态管理由 [`interrupt_state`] crate 提供，本 crate re-export 便捷访问。
//!
//! ## Per-CPU 状态
//!
//! | 变量 | 类型 | 说明 |
//! |------|------|------|
//! | `LOCK_STACK` | `LockStack` | 锁获取顺序栈（死锁检测） |

pub mod irq_safe;
pub mod lock_stack;
pub mod mutex;
pub mod raw;

use per_cpu::cpu_local;

/// Per-CPU 锁栈——强制锁获取顺序，防止死锁。
#[cpu_local]
pub static LOCK_STACK: lock_stack::LockStack = lock_stack::LockStack::new();

// ── 类型别名：保持与原 API 完全兼容 ──────────────────────────

/// 自旋锁——不操作中断。
pub type SpinLock<T> = mutex::Mutex<raw::RawSpinLock, T>;

/// 自旋锁 RAII guard。
pub type SpinLockGuard<'a, T> = mutex::MutexGuard<'a, raw::RawSpinLock, T>;

/// 中断安全的自旋锁。
pub type SpinLockIrq<T> = irq_safe::IrqSafe<raw::RawSpinLock, T>;

/// 中断安全的自旋锁 RAII guard。
pub type SpinLockIrqGuard<'a, T> = irq_safe::IrqSafeGuard<'a, raw::RawSpinLock, T>;

// ── 便捷 re-export ──────────────────────────────────────────

pub use interrupt_state::HeldInterrupts;
pub use irq_safe::lock_level;
