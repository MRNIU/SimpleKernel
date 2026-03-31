//! 内核内存管理——帧分配器、页表、堆、MMIO 映射。

#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_os = "none", feature(sync_unsafe_cell))]

#[cfg(any(test, target_os = "none"))]
extern crate alloc;

/// 错误类型。
pub mod error;
/// 物理帧分配器与帧生命周期状态机（re-export `frame_allocator` crate）。
#[cfg(any(test, target_os = "none"))]
pub use frame_allocator as frame;
/// 堆分配器（re-export `heap` crate）。
#[cfg(target_os = "none")]
pub use heap_crate as heap;
/// 全局内存状态。
pub mod globals;
/// 内存子系统初始化（依赖链接器符号，裸机专用）。
#[cfg(target_os = "none")]
pub mod init;
/// NodeFrame 桥接与 PageTable 类型别名。
pub mod node_frame;
/// 虚拟内存区域（VMA）与地址空间管理。
#[cfg(any(test, target_os = "none"))]
pub mod vma;

/// TLB 管理（re-export `tlb` crate）。
pub use tlb;

/// 仿射类型映射——具体化的类型别名。
#[cfg(target_os = "none")]
pub type MappedPages = paging::MappedPages<node_frame::NodeFrame>;
/// 仿射类型映射——测试用类型别名。
#[cfg(test)]
pub type MappedPages = paging::MappedPages<paging::HeapNodeFrame>;

/// MMIO 区域——具体化的类型别名。
#[cfg(target_os = "none")]
pub type MmioRegion = paging::mmio::MmioRegion<node_frame::NodeFrame>;
/// MMIO 区域——测试用类型别名。
#[cfg(test)]
pub type MmioRegion = paging::mmio::MmioRegion<paging::HeapNodeFrame>;

/// 映射错误类型 re-export。
pub use paging::error::PagingError;

// 公共 API re-export——保持外部调用方的 `memory::Xxx` 路径不变。
pub use globals::{MEMORY_INFO, MemoryInfo};
#[cfg(any(test, target_os = "none"))]
pub use globals::{
    kernel_address_space, kernel_page_table, store_kernel_address_space, store_kernel_page_table,
};

#[cfg(any(test, target_os = "none"))]
pub use address::{phys_to_virt, virt_to_phys};

#[cfg(target_os = "none")]
pub use init::{init, init_smp};

/// 将 MMIO 物理地址区间 identity-map 到内核页表，返回对应虚拟地址。
///
/// 映射标记为永久（drop 时不 unmap），并在内核地址空间中注册 VMA 记录。
/// 如需 RAII 管理的 MMIO 映射，请使用 [`MmioRegion::map_to`]。
///
/// # Errors
///
/// 内核页表未初始化或映射冲突时返回错误。
///
/// # Panics
///
/// 内核地址空间中注册 VMA 失败时 panic（通常是重复映射同一 MMIO 区域）。
#[cfg(any(test, target_os = "none"))]
pub fn map_mmio(
    paddr: address::PhysAddr,
    size: usize,
) -> Result<address::VirtAddr, error::MemoryError> {
    use paging::{PteFlags, PteFlagsOps};

    let kpt = kernel_page_table().ok_or(error::MemoryError::InvalidPageTable)?;
    let region = MmioRegion::map_to(kpt, paddr, size)?;
    let vaddr = region.base();
    let region_size = region.size();
    let _permanent = region.into_permanent();

    // 在内核地址空间中注册 MMIO 区域
    if let Some(kas) = kernel_address_space() {
        kas.lock()
            .register_existing(
                vaddr,
                region_size,
                PteFlags::kernel_device(),
                vma::VmaKind::Identity,
            )
            .expect("MMIO 区域注册到内核地址空间失败——可能是重复映射");
    }

    Ok(vaddr)
}
