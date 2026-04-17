//! 物理帧分配器——RAII 帧所有权 + buddy 后端。
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
//! # 分配器接口
//!
//! ```text
//! buddy pool ──alloc()──▶ AllocatedFrames ──Drop──▶ buddy pool
//! ```
//!
//! `AllocatedFrames` 是唯一的公开帧类型——持有所有权，Drop 时归还分配器。
//! 帧内容**未初始化**，调用方按需清零或初始化。
//!
//! 帧的所有权通过 Rust move 语义在编译期追踪，无需引用计数或 PTE 标记位。
//!
//! # 典型用法
//!
//! ```rust,ignore
//! let frames = AllocatedFrames::alloc(4)?;  // 4 连续帧，内容未初始化
//! // frames 通过 identity mapping 可直接访问
//! let ptr: *mut u8 = frames.start_paddr().to_virt().as_mut_ptr();
//! // Drop 时自动归还 buddy
//! ```

#![no_std]

mod alloc;
mod error;
mod frames;

pub use alloc::init;
pub use error::FrameAllocError;
pub use frames::AllocatedFrames;

/// 物理帧范围——`frame_allocator` 内部使用的便利别名。
pub(crate) type FrameSpan = memory_types::Span<memory_types::Frame>;
