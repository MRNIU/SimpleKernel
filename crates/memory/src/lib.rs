//! 内核内存管理门面——统一编排子系统初始化、提供 MMIO 映射接口。
//!
//! # 架构总览
//!
//! 内存子系统由多个 crate 分三层协同工作：
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │  策略层 — memory (本 crate)                          │
//! │  init() · init_smp() · map_mmio() · MMIO 跟踪       │
//! ├─────────────────────────────────────────────────────┤
//! │  机制层 — paging                                     │
//! │  PageTable · OwnedPages · MmioRegion                 │
//! ├───────────────┬──────────────────┬──────────────────┤
//! │ frame_allocator│ page_table_entry  │ tlb             │
//! │ 物理帧分配     │ PTE 编解码        │ TLB 刷新        │
//! │ Frames<S>     │ PteFlagsOps/PteOps│ TlbFlushGuard   │
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
//! // MMIO 映射
//! let va = memory::map_mmio(PhysAddr::new(0x1000_0000), 0x1000)?;
//! ```
//!
//! # 初始化顺序
//!
//! 1. `heap::init()` — 启用堆分配（buddy 内部需要 BTreeSet）
//! 2. `frame_allocator::init()` — 空闲帧入 buddy，内核段帧预留
//! 3. `PageTable::create()` + `identity_map_range()` — 背景层
//! 4. `OwnedPages::new()` × 3 + `mem::forget()` — 覆盖层（永久持有）
//!
//! 详见 `docs/design/memory-subsystem-v2.md`。

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;

use memory_types::VirtAddr;
use sync_crate::SpinLockIrq;

/// 错误类型。
pub mod error;
/// 物理帧分配器（re-export `frame_allocator` crate）。
pub use frame_allocator as frame;
/// 堆分配器（re-export `heap` crate）。
pub use heap_crate as heap;
/// 全局内存状态。
pub mod globals;
/// 内存子系统初始化（依赖链接器符号，裸机专用）。
pub mod init;

/// TLB 管理（re-export `tlb` crate）。
pub use tlb;

/// 映射错误类型 re-export。
pub use paging::error::PagingError;

pub use globals::{MEMORY_INFO, MemoryInfo};

pub use init::{init, init_smp};

/// 已注册的 MMIO 区域——用于检测重叠。
///
/// 以基地址为键、区域大小为值。无需 `PteFlags`——
/// MMIO 区域始终使用 `kernel_device()` 权限。
static MMIO_REGIONS: SpinLockIrq<BTreeMap<VirtAddr, usize>> = SpinLockIrq::new(
    BTreeMap::new(),
    "mmio_regions",
    sync_crate::lock_level::UNSPECIFIED,
);

/// 将 MMIO 物理地址区间 identity-map，返回 `paddr` 对应的虚拟地址。
///
/// 内部按页对齐建立映射，但返回值精确对应调用方请求的 `paddr`（类似 Linux `ioremap`）。
/// MMIO 映射永久存在（MmioRegion 不 unmap）。
///
/// # Errors
///
/// 页表映射失败时返回错误。
pub fn map_mmio(
    paddr: memory_types::PhysAddr,
    size: usize,
) -> Result<memory_types::VirtAddr, error::MemoryError> {
    let region = paging::mmio::MmioRegion::map(paddr, size)?;
    let region_base = region.base();
    let region_size = region.size();

    // 多个 MMIO 设备可能落在同一 4KB 页内（如 QEMU virtio,mmio 每 0x200 字节一个），
    // 页对齐后区域完全相同——MmioIdentical 表示已注册，安全跳过。
    // 部分重叠（MmioOverlap）是真正的冲突，必须 panic。
    match check_mmio_overlap(region_base, region_size) {
        Ok(()) => {}
        Err(error::MemoryError::MmioIdentical) => {}
        Err(e) => panic!("MMIO 区域注册失败: {e}"),
    }

    // 返回 paddr 对应的精确虚拟地址（identity mapping: VA == PA）
    Ok(paddr.to_virt())
}

/// 检查并注册 MMIO 区域——检测重叠冲突。
fn check_mmio_overlap(base: VirtAddr, size: usize) -> Result<(), error::MemoryError> {
    use memory_types::Span;

    let new_range = Span::new(base, base + size);
    let mut regions = MMIO_REGIONS.lock();

    // 检查与已有区域的重叠
    for (&existing_base, &existing_size) in regions.iter() {
        let existing_range = Span::new(existing_base, existing_base + existing_size);
        if new_range == existing_range {
            return Err(error::MemoryError::MmioIdentical);
        }
        if new_range.overlaps(existing_range) {
            return Err(error::MemoryError::MmioOverlap);
        }
    }

    regions.insert(base, size);
    Ok(())
}
