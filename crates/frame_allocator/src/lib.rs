//! 物理帧分配器 + typestate 生命周期追踪。

#![no_std]
#![feature(adt_const_params)]

mod alloc;
mod error;
mod state;
mod transitions;

pub use alloc::init;
pub use error::FrameAllocError;
pub use state::{AllocatedFrames, FrameState, Frames};

/// 物理帧范围——`frame_allocator` 内部使用的便利别名。
pub(crate) type FrameSpan = memory_types::Span<memory_types::Frame>;
