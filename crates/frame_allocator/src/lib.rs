//! 物理帧分配器——typestate 生命周期追踪 + buddy 后端。
//!
//! # 在内存子系统中的定位
//!
//! 本 crate 是内存子系统的**资源层**——管理物理帧的分配和回收，
//! 不感知页表的存在。帧的权限管理由上层 `paging::OwnedPages` 负责。
//!
//! ```text
//! paging::OwnedPages (权限管理)
//!    │
//!    ▼ 持有 AllocatedFrames 字段
//! frame_allocator (本 crate: 帧所有权追踪)
//!    │
//!    ▼ alloc/dealloc
//! buddy_system_allocator (后端)
//! ```
//!
//! # 2-State Typestate
//!
//! ```text
//! buddy pool ──alloc()──▶ Allocated ──Drop──▶ buddy pool
//! ```
//!
//! - `Free`：分配器持有，仅在 `alloc_from_backend` 内部短暂存在
//! - `Allocated`：外部持有，通过 `AllocatedFrames` 类型表示
//!
//! 帧的所有权通过 Rust move 语义在编译期追踪，无需引用计数或 PTE 标记位。
//!
//! # 典型用法
//!
//! ```rust,ignore
//! let frames = AllocatedFrames::alloc(4)?;  // 4 连续帧，已清零
//! // frames 通过 identity mapping 可直接访问
//! let ptr: *mut u8 = frames.start_paddr().to_virt().as_mut_ptr();
//! // Drop 时自动归还 buddy
//! ```

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
