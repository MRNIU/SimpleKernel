//! QEMU VirtIO 恒等映射 DMA 后端。

extern crate alloc;

use alloc::collections::BTreeMap;
use core::alloc::Layout;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

use dma_api::{DmaAddr, DmaHandle, DmaMapHandle, DmaOp};
use frame_allocator::AllocatedFrames;
use memory_types::VirtAddr;
use sync_crate::SpinLock;

use crate::{DmaDirection, DmaError, DmaResult};

/// QEMU VirtIO identity-mapped DMA 操作实现。
pub struct QemuIdentityDmaOp;

pub(crate) static QEMU_IDENTITY_DMA_OP: QemuIdentityDmaOp = QemuIdentityDmaOp;

static DMA_TRACKER: SpinLock<BTreeMap<u64, AllocatedFrames>> =
    SpinLock::new(BTreeMap::new(), "dma_tracker", sync_crate::lock_level::DMA);

impl DmaOp for QemuIdentityDmaOp {
    fn page_size(&self) -> usize {
        config::PAGE_SIZE
    }

    unsafe fn map_single(
        &self,
        _dma_mask: u64,
        addr: NonNull<u8>,
        size: NonZeroUsize,
        align: usize,
        _direction: dma_api::DmaDirection,
    ) -> Result<DmaMapHandle, dma_api::DmaError> {
        let vaddr = VirtAddr::new(addr.as_ptr() as usize);
        let paddr = vaddr.to_phys().as_usize() as u64;
        let layout = Layout::from_size_align(size.get(), align)?;

        // SAFETY: 当前内核使用 SAS identity mapping；`addr` 指向调用方提供的连续
        // buffer，`paddr` 是同一区域的设备可见地址，layout 来自调用方 size/align。
        Ok(unsafe { DmaMapHandle::new(addr, DmaAddr::from(paddr), layout, None) })
    }

    unsafe fn unmap_single(&self, _handle: DmaMapHandle) {}

    unsafe fn alloc_coherent(&self, _dma_mask: u64, layout: Layout) -> Option<DmaHandle> {
        let pages = layout.size().div_ceil(config::PAGE_SIZE).max(1);
        let frames = AllocatedFrames::alloc(pages).ok()?;
        let paddr = frames.start_paddr();
        let vaddr = paddr.to_virt();
        let ptr = NonNull::new(vaddr.as_mut_ptr::<u8>())?;

        // SAFETY: identity mapping 下 PA.to_virt() 有效；帧刚分配，尚无其他引用。
        unsafe {
            core::ptr::write_bytes(ptr.as_ptr(), 0, pages * config::PAGE_SIZE);
        }

        DMA_TRACKER.lock().insert(paddr.as_usize() as u64, frames);

        // SAFETY: `ptr` 指向刚分配并清零的连续 DMA 内存；DMA 地址与 identity
        // mapping 下的物理地址一致；layout 由调用方 `dma-api` 校验。
        Some(unsafe { DmaHandle::new(ptr, DmaAddr::from(paddr.as_usize() as u64), layout) })
    }

    unsafe fn dealloc_coherent(&self, handle: DmaHandle) {
        let paddr = handle.dma_addr().as_u64();
        if DMA_TRACKER.lock().remove(&paddr).is_none() {
            log::error!("dma dealloc: 未找到 paddr={paddr:#x} 的 DMA 分配记录");
        }
    }
}

/// 分配页级 raw coherent DMA 区域，供 `virtio-drivers::Hal::dma_alloc` 使用。
pub fn raw_alloc_pages(pages: usize, _direction: DmaDirection) -> DmaResult<(u64, NonNull<u8>)> {
    if pages == 0 {
        return Err(DmaError::ZeroPages);
    }

    let layout = Layout::from_size_align(pages * config::PAGE_SIZE, config::PAGE_SIZE)
        .map_err(DmaError::from)?;
    // SAFETY: layout 由非零页数和 PAGE_SIZE 构造，满足页对齐；当前 QEMU 后端不使用
    // dma_mask，分配出的连续帧由 DMA_TRACKER 持有并由 raw_dealloc_pages 配对释放。
    let handle = unsafe { QEMU_IDENTITY_DMA_OP.alloc_coherent(u64::MAX, layout) }
        .ok_or(DmaError::NoMemory)?;

    Ok((handle.dma_addr().as_u64(), handle.as_ptr()))
}

/// 释放页级 raw coherent DMA 区域。
pub fn raw_dealloc_pages(paddr: u64, vaddr: NonNull<u8>, pages: usize) -> DmaResult<()> {
    if pages == 0 {
        return Err(DmaError::ZeroPages);
    }

    let mut tracker = DMA_TRACKER.lock();
    let frames = tracker
        .remove(&paddr)
        .ok_or(DmaError::UnknownRawRegion { paddr })?;
    let actual_pages = frames.page_count();
    if actual_pages != pages {
        tracker.insert(paddr, frames);
        return Err(DmaError::RawRegionPageMismatch {
            paddr,
            expected_pages: pages,
            actual_pages,
        });
    }
    let expected_vaddr = frames.start_paddr().to_virt().as_usize();
    let actual_vaddr = vaddr.as_ptr() as usize;
    if expected_vaddr != actual_vaddr {
        tracker.insert(paddr, frames);
        return Err(DmaError::RawRegionVirtualAddressMismatch {
            paddr,
            expected_vaddr,
            actual_vaddr,
        });
    }

    Ok(())
}

/// 映射已有 buffer 为设备可见 DMA 地址。
///
/// # Safety
///
/// `buffer` 必须在 DMA 共享期间指向有效的连续内存区域，并且不能以违反
/// VirtIO HAL 契约的方式被并发修改。
pub unsafe fn raw_map_single(buffer: NonNull<[u8]>, direction: DmaDirection) -> DmaResult<u64> {
    // SAFETY: `virtio-drivers::Hal::share` 的调用方保证 buffer 在共享期间有效。
    let slice = unsafe { buffer.as_ref() };
    let size = NonZeroUsize::new(slice.len()).ok_or(DmaError::ZeroSizedBuffer)?;
    let ptr =
        NonNull::new(slice.as_ptr() as *mut u8).ok_or(DmaError::NullVirtualAddress { paddr: 0 })?;

    // SAFETY: ptr 和 size 来自调用方提供的非空 slice；raw_map_single 的 Safety 契约要求
    // buffer 在 DMA 共享期间保持有效且连续。
    let handle =
        unsafe { QEMU_IDENTITY_DMA_OP.map_single(u64::MAX, ptr, size, 1, direction.into()) }
            .map_err(DmaError::from_api)?;
    Ok(handle.dma_addr().as_u64())
}

/// 解除已有 buffer 的 streaming DMA 映射。
pub fn raw_unmap_single(_paddr: u64, _buffer: NonNull<[u8]>, _direction: DmaDirection) {}
