//! 内核内存管理——帧分配器、页分配器、页表、堆、MMIO 映射。

#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_os = "none", feature(sync_unsafe_cell))]

#[cfg(any(test, target_os = "none"))]
extern crate alloc;

/// 错误类型。
pub mod error;
/// 物理帧分配器（re-export `frame_allocator` crate）。
#[cfg(any(test, target_os = "none"))]
pub use frame_allocator as frame;
/// 堆分配器（re-export `heap` crate）。
#[cfg(target_os = "none")]
pub use heap_crate as heap;
/// 虚拟页分配器（re-export `page_allocator` crate）。
#[cfg(any(test, target_os = "none"))]
pub use page_allocator as page;
/// 全局内存状态。
pub mod globals;
/// 内存子系统初始化（依赖链接器符号，裸机专用）。
#[cfg(target_os = "none")]
pub mod init;
/// PageTable 类型 re-export。
pub mod node_frame;
/// 虚拟内存区域（VMA）与地址空间管理。
#[cfg(any(test, target_os = "none"))]
pub mod vma;

/// TLB 管理（re-export `tlb` crate）。
pub use tlb;

/// 仿射类型映射——re-export `paging::MappedPages`。
#[cfg(any(test, target_os = "none"))]
pub type MappedPages = paging::MappedPages;

/// MMIO 区域——re-export `paging::mmio::MmioRegion`。
#[cfg(any(test, target_os = "none"))]
pub type MmioRegion = paging::mmio::MmioRegion;

/// 映射错误类型 re-export。
pub use paging::error::PagingError;

pub use globals::{MEMORY_INFO, MemoryInfo};
#[cfg(any(test, target_os = "none"))]
pub use globals::{kernel_address_space, store_kernel_address_space};

#[cfg(any(test, target_os = "none"))]
pub use memory_types::{phys_to_virt, virt_to_phys};

#[cfg(target_os = "none")]
pub use init::{init, init_smp};

/// 将 MMIO 物理地址区间 identity-map，返回对应虚拟地址。
///
/// MmioRegion 建立映射后通过 `forget` 阻止 pages 回到 page_allocator，
/// 并在内核地址空间中注册 VMA 记录。
///
/// # Errors
///
/// 页/帧分配或映射失败时返回错误。
#[cfg(any(test, target_os = "none"))]
pub fn map_mmio(
    paddr: memory_types::PhysAddr,
    size: usize,
) -> Result<memory_types::VirtAddr, error::MemoryError> {
    use paging::{PteFlags, PteFlagsOps};

    let region = MmioRegion::map(paddr, size)?;
    let vaddr = region.base();
    let region_size = region.size();

    if let Some(kas) = kernel_address_space() {
        kas.lock()
            .register_existing(
                vaddr,
                region_size,
                PteFlags::kernel_device(),
                vma::VmaKind::Identity,
            )
            .expect("MMIO 区域注册到内核地址空间失败");
    }

    // 阻止 MmioRegion drop 归还 pages 给 page_allocator——MMIO 映射永久存在
    core::mem::forget(region);

    Ok(vaddr)
}
