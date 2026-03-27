//! 统一 `alloc` / `std` 条件导入。
//!
//! 内核在 `no_std` 下使用 `alloc` crate；`cargo test` 在宿主机运行时使用 `std`。
//! 此模块集中定义这些条件导入，避免各模块重复 `#[cfg(test)]` / `#[cfg(not(test))]`。

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
