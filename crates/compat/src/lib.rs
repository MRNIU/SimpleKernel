#![cfg_attr(not(test), no_std)]

//! 统一 `alloc` / `std` 条件导入。
//!
//! 内核在 `no_std` 下使用 `alloc` crate；`cargo test` 在宿主机运行时使用 `std`。

#[cfg(not(test))]
extern crate alloc;

#[cfg(not(test))]
pub use alloc::collections::VecDeque;
#[cfg(test)]
pub use std::collections::VecDeque;

#[cfg(not(test))]
pub use alloc::sync::Arc;
#[cfg(test)]
pub use std::sync::Arc;

#[cfg(not(test))]
pub use alloc::vec::Vec;
#[cfg(test)]
pub use std::vec::Vec;
