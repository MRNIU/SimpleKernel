#![cfg_attr(not(test), no_std)]

//! 内核同步原语——SpinLock（不关中断）、SpinLockIrq（关中断）和 HeldInterrupts 证明令牌。

pub mod interrupt_ops;
pub mod spinlock;

pub use interrupt_ops::HeldInterrupts;
pub use spinlock::SpinLock;
pub use spinlock::SpinLockGuard;
pub use spinlock::SpinLockIrq;
pub use spinlock::SpinLockIrqGuard;
