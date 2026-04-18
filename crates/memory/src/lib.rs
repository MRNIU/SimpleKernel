//! 内核内存管理门面——统一编排子系统初始化、提供 MMIO 映射接口。
//!
//! # 架构总览
//!
//! 内存子系统由多个 crate 分三层协同工作：
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │  策略层 — memory (本 crate)                          │
//! │  init() · init_smp() · map_mmio()                    │
//! ├─────────────────────────────────────────────────────┤
//! │  机制层 — paging                                     │
//! │  PageTable · OwnedPages · MmioRegion                 │
//! ├───────────────┬──────────────────┬──────────────────┤
//! │ frame_allocator│ page_table_entry  │ tlb             │
//! │ 物理帧分配     │ PTE 编解码        │ TLB 刷新        │
//! │ AllocatedFrames│ PteFlagsOps/PteOps│ TlbFlushGuard   │
//! ├───────────────┴──────────────────┴──────────────────┤
//! │  memory_types — PhysAddr · VirtAddr · Frame · Span   │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # SAS 全量映射模型
//!
//! SAS 架构下所有物理内存在 boot 时 identity-map 为 `kernel_rw`（背景层），
//! 运行时只调整权限（覆盖层），永远不创建或删除 PTE。
//!
//! # 错误策略
//!
//! SAS 下分页/MMIO 映射失败都是**内核 bug**（boot 时 OOM、RAM/Device 重叠、
//! flags 冲突），因此 [`map_mmio`] 和下层页表操作**失败即 panic**。
//! 唯一向上传递 `FrameAllocError::OutOfMemory` 的场景是运行时帧分配
//! （`AllocatedFrames::alloc`），不经过本 crate 的接口。
//!
//! # 典型使用方式
//!
//! ```rust,ignore
//! use memory::frame::AllocatedFrames;
//! use paging::{OwnedPages, PteFlags, PteFlagsOps};
//!
//! // 分配帧 + 设置只读权限
//! let frames = AllocatedFrames::alloc(4)?;
//! let mapping = OwnedPages::new(frames, PteFlags::kernel_ro());
//! // ... 使用 mapping.vaddr() 访问内存 ...
//! drop(mapping); // 恢复 kernel_rw + 归还帧
//!
//! // MMIO 映射（失败 panic）
//! let region = memory::map_mmio(PhysAddr::new(0x1000_0000), 0x1000);
//! ```

#![no_std]

/// 物理帧分配器 re-export。
///
/// **不要让 `simplekernel` 根 crate 直接依赖 `frame_allocator`**——已观察到
/// 在根 `Cargo.toml` 添加 `frame_allocator = { path = ... }` 会导致 SMP QEMU
/// 启动后立即死锁（QEMU 100% CPU、无任何串口输出），疑似 workspace 依赖图
/// 在多测试二进制场景下产生的 linker 边界情况。根因待调查（见
/// `docs/audit/audit-progress.md` 中"待办：调查 frame_allocator 直接依赖死锁"）。
pub use frame_allocator as frame;
/// 堆分配器 re-export。
pub use heap_crate as heap;
/// TLB 管理 re-export。
pub use tlb;

/// 全局内存状态。
pub mod globals;
/// 内存子系统初始化（依赖链接器符号，裸机专用）。
pub mod init;

/// MMIO 区域（re-export `paging::mmio::MmioRegion`，是 `map_mmio` 的返回类型）。
pub use paging::mmio::MmioRegion;

pub use globals::{MEMORY_INFO, MemoryInfo};

pub use init::{init, init_smp};

/// 将 MMIO 物理地址区间 identity-map，返回类型安全的 [`MmioRegion`]。
///
/// 这是建立 MMIO 映射的**唯一公开入口**——内部完成 RAM 重叠校验和页表映射。
/// MMIO 映射永久存在（MmioRegion 不 unmap）。
///
/// # Panics
///
/// - `paddr + size` 与 RAM 范围重叠（拒绝将 RAM 重映射为 Device 内存）
/// - 页表映射冲突或节点 OOM
pub fn map_mmio(paddr: memory_types::PhysAddr, size: usize) -> MmioRegion {
    let info = MEMORY_INFO.get().expect("map_mmio: MEMORY_INFO 未初始化");
    let ram_start = info.physical_memory_addr.as_usize();
    let ram_end = ram_start + info.physical_memory_size;
    assert!(
        paddr.as_usize() + size <= ram_start || paddr.as_usize() >= ram_end,
        "map_mmio: paddr {} + {:#x} 落在 RAM 范围 [{:#x}, {:#x})——拒绝映射为 Device 内存",
        paddr,
        size,
        ram_start,
        ram_end
    );

    paging::mmio::MmioRegion::map(paddr, size)
}
