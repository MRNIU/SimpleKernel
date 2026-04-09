//! 物理帧分配器 + typestate 生命周期追踪。

#![no_std]
#![allow(incomplete_features)]
#![feature(adt_const_params)]

mod alloc;
mod backend;
mod bitmap;
mod error;
mod state;
mod transitions;

pub use alloc::{alloc_from_backend, init};
pub use error::FrameAllocError;
pub use state::{AllocatedFrames, Frames, FreeFrames, MappedFrames, MemoryState, UnmappedFrames};
