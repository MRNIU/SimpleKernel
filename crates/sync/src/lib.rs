#![cfg_attr(not(test), no_std)]

//! 内核同步原语——中断安全的 SpinLock 和 HeldInterrupts 证明令牌。

pub mod interrupt_ops;
pub mod spinlock;

pub use interrupt_ops::HeldInterrupts;
pub use spinlock::SpinLock;
pub use spinlock::SpinLockGuard;
