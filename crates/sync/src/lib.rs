#![cfg_attr(not(test), no_std)]

//! 内核同步原语——SpinLock（不关中断）、SpinLockIrq（关中断）和 HeldInterrupts 证明令牌。
//!
//! ## Per-CPU 状态
//!
//! | 变量 | 类型 | 说明 |
//! |------|------|------|
//! | `LOCK_STACK` | `LockStack` | 锁获取顺序栈（死锁检测） |

pub mod interrupt_ops;
pub mod irq;
pub mod lock_stack;
pub mod spinlock;

use per_cpu::cpu_local;

/// Per-CPU 锁栈——强制锁获取顺序，防止死锁。
#[cpu_local]
pub static LOCK_STACK: lock_stack::LockStack = lock_stack::LockStack::new();

pub use interrupt_ops::HeldInterrupts;
pub use irq::{irq_disable, irq_enable, irq_enabled};
pub use spinlock::SpinLock;
pub use spinlock::SpinLockGuard;
pub use spinlock::SpinLockIrq;
pub use spinlock::SpinLockIrqGuard;
