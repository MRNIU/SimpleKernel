//! 内核内存管理门面——统一编排子系统初始化、提供 MMIO 映射接口。

#![no_std]

pub use heap_crate as heap;
pub use tlb;

pub mod globals;
pub mod init;

pub use globals::{MEMORY_INFO, MemoryInfo};
pub use init::{init, init_smp};
pub use paging::mmio::MmioRegion;

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
