//! 类型化 MMIO 区域——强制 volatile 语义的映射包装。
//!
//! `MappedPages` 使用普通内存语义（non-volatile），适合 RAM 映射。
//! `MmioRegion` 使用 volatile 语义，适合设备寄存器——编译器不会优化掉
//! 对同一地址的重复读写，也不会重排 MMIO 操作。
//!
//! 内部使用 `ManuallyDrop<MappedPages>` 阻止自动 unmap——
//! MMIO 区域的生命周期通常等于设备驱动的生命周期。

use core::mem::ManuallyDrop;

use crate::error::PagingError;
use crate::mapping::{MappedPages, check_bounds_and_align};
use crate::{PteFlags, PteFlagsOps};
use address::PhysAddr;
use config::PAGE_SIZE;
use page_allocator::AllocatedPages;

/// 已映射的 MMIO 区域——提供类型安全的 volatile 寄存器访问。
///
/// 内部通过 `ManuallyDrop<MappedPages>` 管理映射——不会自动 unmap。
/// 不可 Clone（一个映射只有一个 owner），可通过 `&self` 共享读取。
pub struct MmioRegion {
    mapping: ManuallyDrop<MappedPages>,
}

impl MmioRegion {
    /// 将 `[paddr, paddr+size)` identity-map，返回 `MmioRegion`。
    ///
    /// # Errors
    ///
    /// 虚拟页分配或映射失败时返回错误。
    pub fn map(paddr: PhysAddr, size: usize) -> Result<Self, PagingError> {
        let pa_aligned = paddr.align_down();
        let page_count = ((paddr + size).align_up().as_usize() - pa_aligned.as_usize()) / PAGE_SIZE;
        let va = address::VirtAddr::new(pa_aligned.as_usize());
        let pages =
            AllocatedPages::alloc_at(va, page_count).map_err(|_| PagingError::AllocationFailed)?;
        let mp = MappedPages::map_identity(pages, PteFlags::kernel_device());
        Ok(Self {
            mapping: ManuallyDrop::new(mp),
        })
    }

    /// 返回 MMIO 区域的基地址。
    #[must_use]
    pub fn base(&self) -> address::VirtAddr {
        self.mapping.vaddr()
    }

    /// 返回 MMIO 区域的大小。
    #[must_use]
    pub fn size(&self) -> usize {
        self.mapping.size()
    }

    /// 读取指定偏移处的寄存器值（volatile 语义）。
    #[inline]
    pub fn read_reg<T: zerocopy::FromBytes>(&self, offset: usize) -> T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.mapping.vaddr().as_usize(),
            self.mapping.size(),
            offset,
            "MmioRegion::read_reg",
        );
        // SAFETY: MmioRegion 构造保证地址区间已映射为 device memory；
        // check_bounds_and_align 验证了越界和对齐；FromBytes 保证任意位模式合法
        unsafe { core::ptr::read_volatile(ptr) }
    }

    /// 写入指定偏移处的寄存器值（volatile 语义）。
    #[inline]
    pub fn write_reg<T: zerocopy::IntoBytes>(&self, offset: usize, val: T) {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.mapping.vaddr().as_usize(),
            self.mapping.size(),
            offset,
            "MmioRegion::write_reg",
        );
        // SAFETY: MmioRegion 构造保证地址区间已映射为 device memory
        unsafe { core::ptr::write_volatile(ptr as *mut T, val) }
    }
}

impl core::fmt::Debug for MmioRegion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MmioRegion({}, size={:#x})",
            self.mapping.vaddr(),
            self.mapping.size(),
        )
    }
}
