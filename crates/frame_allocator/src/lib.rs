//! 物理帧分配器 + typestate 生命周期追踪。

#![no_std]
#![expect(
    incomplete_features,
    reason = "adt_const_params 尚未稳定，MemoryState 作为 const generic 参数需要此 feature"
)]
#![feature(adt_const_params)]

mod alloc;
mod error;
mod state;
mod transitions;

pub use alloc::init;
pub use error::FrameAllocError;
pub use state::{AllocatedFrames, Frames, MappedFrames, MemoryState, UnmappedFrames};
