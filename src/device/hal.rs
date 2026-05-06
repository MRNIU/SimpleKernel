//! `virtio-drivers` crate 的 HAL 实现——连接内核内存管理与 VirtIO 协议栈。
//!
//! 通过实现 [`virtio_drivers::Hal`] trait，使 VirtIO crate 能够：
//! - 通过 `crates/dma` 分配/释放 DMA 缓冲区（物理连续、已清零）
//! - 将 MMIO 物理地址转换为可访问的虚拟地址
//! - 通过 `crates/dma` 共享/取消共享缓冲区
//!
//! 当前 QEMU 后端仍采用 identity mapping，HAL 层只负责把 VirtIO 的方向语义转换为
//! SimpleKernel 的 DMA 方向语义。
//!
//! [VirtIO spec §2.6](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)

use core::ptr::NonNull;

use memory_types::PhysAddr;
use virtio_drivers::{BufferDirection, Hal};

/// SimpleKernel 的 VirtIO HAL 实现。
pub struct SimpleKernelHal;

fn virtio_direction(direction: BufferDirection) -> dma::DmaDirection {
    match direction {
        BufferDirection::DriverToDevice => dma::DmaDirection::ToDevice,
        BufferDirection::DeviceToDriver => dma::DmaDirection::FromDevice,
        BufferDirection::Both => dma::DmaDirection::Bidirectional,
    }
}

// SAFETY: 实现满足 Hal trait 的所有安全约束：
// - dma_alloc 委托 `crates/dma` 返回页对齐、已清零、不与其他分配别名的指针
// - mmio_phys_to_virt 返回有效的虚拟地址指针
// - share/unshare 委托 `crates/dma` 完成流式 DMA 映射；当前 QEMU 后端为 identity mapping
unsafe impl Hal for SimpleKernelHal {
    /// 分配 DMA 缓冲区——委托 `crates/dma` 管理帧所有权与后端映射。
    fn dma_alloc(pages: usize, direction: BufferDirection) -> (u64, NonNull<u8>) {
        let direction = virtio_direction(direction);
        dma::raw_alloc_pages(pages, direction).unwrap_or_else(|error| {
            panic!("DMA 帧分配失败: pages={pages}, direction={direction:?}, error={error}");
        })
    }

    /// 释放 DMA 缓冲区——委托 `crates/dma` 释放对应页级 DMA 区域。
    ///
    /// # Safety
    ///
    /// `paddr`、`vaddr` 和 `pages` 必须与 `dma_alloc` 返回值匹配；底层
    /// `raw_dealloc_pages` 会按分配记录校验虚拟地址。
    unsafe fn dma_dealloc(paddr: u64, vaddr: NonNull<u8>, pages: usize) -> i32 {
        match dma::raw_dealloc_pages(paddr, vaddr, pages) {
            Ok(()) => 0,
            Err(error) => {
                log::error!("dma_dealloc: paddr={paddr:#x}, pages={pages}, error={error}");
                -1
            }
        }
    }

    /// MMIO 物理地址转虚拟地址。
    ///
    /// Identity mapping 下直接调用 `PhysAddr::to_virt`。
    /// 调用方（virtio-drivers crate）保证 paddr 落在有效 MMIO 区域内。
    ///
    /// # Safety
    ///
    /// `paddr` 和 `size` 必须描述有效的 MMIO 区域。
    unsafe fn mmio_phys_to_virt(paddr: u64, _size: usize) -> NonNull<u8> {
        let vaddr = PhysAddr::new(paddr as usize).to_virt();
        NonNull::new(vaddr.as_mut_ptr::<u8>()).expect("MMIO vaddr 为空")
    }

    /// 共享缓冲区——委托 `crates/dma` 建立流式 DMA 映射。
    ///
    /// # Safety
    ///
    /// 缓冲区在共享期间不被其他线程访问。
    unsafe fn share(buffer: NonNull<[u8]>, direction: BufferDirection) -> u64 {
        let direction = virtio_direction(direction);
        // SAFETY: `virtio-drivers::Hal::share` 的调用契约保证 buffer 是有效且非空的
        // DMA 缓冲区，并提供 `raw_map_single` 所需的独占访问和生命周期保证。
        unsafe { dma::raw_map_single(buffer, direction) }.unwrap_or_else(|error| {
            panic!("DMA buffer 共享失败: direction={direction:?}, error={error}");
        })
    }

    /// 取消共享缓冲区——委托 `crates/dma` 释放流式 DMA 映射。
    ///
    /// # Safety
    ///
    /// `buffer` 必须有效且非空，调用期间不能被并发访问，并且必须和 `paddr`、
    /// `direction` 一起对应之前的 `share` 映射。
    unsafe fn unshare(paddr: u64, buffer: NonNull<[u8]>, direction: BufferDirection) {
        dma::raw_unmap_single(paddr, buffer, virtio_direction(direction));
    }
}
