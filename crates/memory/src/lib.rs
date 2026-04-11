//! 内核内存管理——帧分配器、页表、堆、MMIO 映射。

#![no_std]

extern crate alloc;

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
/// 虚拟内存区域（VMA）与地址空间管理。
pub mod vma;

/// TLB 管理（re-export `tlb` crate）。
pub use tlb;

/// 仿射类型帧所有权——re-export `paging::OwnedPages`。
pub type OwnedPages = paging::OwnedPages;

/// MMIO 区域——re-export `paging::mmio::MmioRegion`。
pub type MmioRegion = paging::mmio::MmioRegion;

/// 映射错误类型 re-export。
pub use paging::error::PagingError;

pub use globals::{MEMORY_INFO, MemoryInfo, kernel_address_space, store_kernel_address_space};

pub use init::{init, init_smp};

/// 将 MMIO 物理地址区间 identity-map，返回 `paddr` 对应的虚拟地址。
///
/// 内部按页对齐建立映射，但返回值精确对应调用方请求的 `paddr`（类似 Linux `ioremap`）。
/// 在内核地址空间中注册 VMA 记录。MMIO 映射永久存在（MmioRegion 不 unmap）。
///
/// # Errors
///
/// 页表映射失败时返回错误。
pub fn map_mmio(
    paddr: memory_types::PhysAddr,
    size: usize,
) -> Result<memory_types::VirtAddr, error::MemoryError> {
    use paging::{PteFlags, PteFlagsOps};

    let region = MmioRegion::map(paddr, size)?;
    let region_base = region.base();
    let region_size = region.size();

    if let Some(kas) = kernel_address_space() {
        // 多个 MMIO 设备可能落在同一 4KB 页内（如 QEMU virtio,mmio 每 0x200 字节一个），
        // 页对齐后 VMA 完全相同——RegionIdentical 表示已注册，安全跳过。
        // 部分重叠（RegionOverlap）是真正的冲突，必须 panic。
        match kas
            .lock()
            .register_existing(region_base, region_size, PteFlags::kernel_device())
        {
            Ok(_) => {}
            Err(error::MemoryError::RegionIdentical) => {}
            Err(e) => panic!("MMIO 区域注册到内核地址空间失败: {e}"),
        }
    }

    // 返回 paddr 对应的精确虚拟地址（identity mapping: VA == PA）
    Ok(paddr.to_virt())
}
