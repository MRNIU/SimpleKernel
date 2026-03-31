//! 类型化 MMIO 区域——强制 volatile 语义的 [`MappedPages`] 包装。
//!
//! `MappedPages` 使用普通内存语义（non-volatile），适合 RAM 映射。
//! `MmioRegion` 使用 volatile 语义，适合设备寄存器——编译器不会优化掉
//! 对同一地址的重复读写，也不会重排 MMIO 操作。

use alloc::sync::Arc;

use crate::error::PagingError;
use crate::mapping::{MappedPages, check_bounds_and_align};
use crate::{NodeFrameOps, PageTable, PteFlags, PteFlagsOps};
use address::PhysAddr;
use config::PAGE_SIZE;
use sync_crate::SpinLock;

/// 已映射的 MMIO 区域——提供类型安全的 volatile 寄存器访问。
///
/// 内部通过 [`MappedPages`] 管理页表映射和生命周期。
/// 不可 Clone（一个映射只有一个 owner），可通过 `&self` 共享读取。
pub struct MmioRegion<F: NodeFrameOps> {
    mapping: MappedPages<F>,
}

impl<F: NodeFrameOps> MmioRegion<F> {
    /// 将 `[paddr, paddr+size)` identity-map 到指定页表，返回 `MmioRegion`。
    ///
    /// # Errors
    ///
    /// 映射失败时返回错误。
    pub fn map_to(
        pt_ref: Arc<SpinLock<PageTable<F>>>,
        paddr: PhysAddr,
        size: usize,
    ) -> Result<Self, PagingError> {
        let pa_aligned = paddr.align_down();
        let page_count = ((paddr + size).align_up().as_usize() - pa_aligned.as_usize()) / PAGE_SIZE;
        let mapping =
            MappedPages::map_identity(pt_ref, pa_aligned, page_count, PteFlags::kernel_device())?;
        Ok(Self { mapping })
    }

    /// 标记为永久映射——drop 时不 unmap。
    ///
    /// 用于 PLIC、GIC 等内核生命周期内永远需要的 MMIO 区域。
    #[must_use]
    pub fn into_permanent(mut self) -> Self {
        self.mapping = self.mapping.into_permanent();
        self
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
    ///
    /// # Panics
    ///
    /// `offset + size_of::<T>()` 超出区域大小、或地址未对齐时 panic。
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
    ///
    /// # Panics
    ///
    /// `offset + size_of::<T>()` 超出区域大小、或地址未对齐时 panic。
    #[inline]
    pub fn write_reg<T: zerocopy::IntoBytes>(&self, offset: usize, val: T) {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.mapping.vaddr().as_usize(),
            self.mapping.size(),
            offset,
            "MmioRegion::write_reg",
        );
        // SAFETY: MmioRegion 构造保证地址区间已映射为 device memory；
        // check_bounds_and_align 验证了越界和对齐；IntoBytes 保证 val 的位模式可安全写入
        unsafe { core::ptr::write_volatile(ptr as *mut T, val) }
    }
}

impl<F: NodeFrameOps> core::fmt::Debug for MmioRegion<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MmioRegion({}, size={:#x})",
            self.mapping.vaddr(),
            self.mapping.size(),
        )
    }
}
