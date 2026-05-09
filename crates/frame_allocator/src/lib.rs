// Copyright The SimpleKernel Contributors

//! 物理帧分配器——RAII 帧所有权 + buddy 后端。
//!
//! `AllocatedFrames` 是唯一的公开帧类型，持有所有权，Drop 时归还分配器。
//! 帧内容**未初始化**，调用方按需清零或初始化。

#![no_std]

mod alloc;
mod error;
mod frames;

pub use alloc::init;
pub use error::FrameAllocError;
pub use frames::AllocatedFrames;

/// 物理帧范围——`frame_allocator` 内部使用的便利别名。
pub(crate) type FrameSpan = memory_types::Span<memory_types::Frame>;
