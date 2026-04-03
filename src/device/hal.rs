//! `virtio-drivers` crate 的 HAL 实现——连接内核内存管理与 VirtIO 协议栈。
//!
//! 通过实现 [`virtio_drivers::Hal`] trait，使 VirtIO crate 能够：
//! - 分配/释放 DMA 缓冲区（物理连续、已清零）
//! - 将 MMIO 物理地址转换为可访问的虚拟地址
//! - 共享/取消共享缓冲区（identity mapping 下为空操作）
//!
//! [VirtIO spec §2.6](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)

use alloc::collections::BTreeMap;

use core::ptr::NonNull;

use memory_types::{PhysAddr, VirtAddr};
use sync::SpinLock;
use virtio_drivers::{BufferDirection, Hal};

use memory::frame::AllocatedFrames;

/// DMA 分配追踪表——存储尚未释放的 DMA 帧，防止 Drop 自动回收。
///
/// key 为 DMA 缓冲区的物理地址（页对齐），value 为持有帧所有权的 `AllocatedFrames`。
static DMA_TRACKER: SpinLock<BTreeMap<u64, AllocatedFrames>> = SpinLock::new(
    BTreeMap::new(),
    "dma_tracker",
    sync::lock_level::UNSPECIFIED,
);

/// SimpleKernel 的 VirtIO HAL 实现。
pub struct SimpleKernelHal;

// SAFETY: 实现满足 Hal trait 的所有安全约束：
// - dma_alloc 返回的指针页对齐、已清零、不与其他分配别名
// - mmio_phys_to_virt 返回有效的虚拟地址指针
// - share/unshare 在 identity mapping 下正确转换地址
unsafe impl Hal for SimpleKernelHal {
    /// 分配 DMA 缓冲区——调用帧分配器获取物理连续页。
    ///
    /// `AllocatedFrames::alloc` 已保证内容清零。
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (u64, NonNull<u8>) {
        let frames = AllocatedFrames::alloc(pages).expect("DMA 帧分配失败");
        let paddr = frames.start_paddr();
        let vaddr = paddr.to_virt();

        // SAFETY: to_virt 保证返回有效的虚拟地址（identity mapping 下 VA == PA）
        let ptr = NonNull::new(vaddr.as_mut_ptr::<u8>()).expect("DMA vaddr 为空");

        // 将帧存入追踪表，阻止 Drop 回收
        DMA_TRACKER.lock().insert(paddr.as_usize() as u64, frames);

        (paddr.as_usize() as u64, ptr)
    }

    /// 释放 DMA 缓冲区——从追踪表移除帧，触发 Drop 归还分配器。
    ///
    /// # Safety
    ///
    /// `paddr` 和 `pages` 必须与 `dma_alloc` 返回值匹配。
    unsafe fn dma_dealloc(paddr: u64, _vaddr: NonNull<u8>, _pages: usize) -> i32 {
        match DMA_TRACKER.lock().remove(&paddr) {
            Some(_frames) => 0, // AllocatedFrames Drop 自动归还
            None => {
                log::error!("dma_dealloc: 未找到 paddr={:#x} 的 DMA 分配记录", paddr);
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

    /// 共享缓冲区——identity mapping 下直接返回物理地址。
    ///
    /// # Safety
    ///
    /// 缓冲区在共享期间不被其他线程访问。
    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> u64 {
        let vaddr = VirtAddr::new(buffer.as_ptr() as *const u8 as usize);
        vaddr.to_phys().as_usize() as u64
    }

    /// 取消共享缓冲区——identity mapping 下无需额外操作。
    ///
    /// # Safety
    ///
    /// `paddr` 必须是对应 `share` 调用返回的值。
    unsafe fn unshare(_paddr: u64, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
        // Identity mapping：无需 IOMMU 解映射或数据回拷
    }
}
