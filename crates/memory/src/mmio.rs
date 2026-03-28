//! 类型化 MMIO 区域——借鉴 Theseus OS 的 `MappedPages::as_type` 模式。
//!
//! `MmioRegion` 封装了一段已映射的 MMIO 地址区域，提供类型安全的
//! 寄存器读写方法，替代裸指针 + `core::ptr::read_volatile` 的传统做法。
//!
//! # 与 `MappedPages` 的区别
//!
//! `MappedPages` 使用普通内存语义（non-volatile），适合 RAM 映射。
//! `MmioRegion` 使用 volatile 语义，适合设备寄存器——编译器不会优化掉
//! 对同一地址的重复读写，也不会重排 MMIO 操作。
//!
//! # 实现
//!
//! 内部通过 [`MappedPages`] 管理页表映射和生命周期，
//! 避免重复实现 unmap / permanent 逻辑。

use crate::error::MemoryError;
use crate::mapped_pages::MappedPages;
use crate::page_table::{PageTable, PteFlags, PteFlagsOps};
use address::PhysAddr;
use sync_crate::SpinLock;

/// 已映射的 MMIO 区域——提供类型安全的寄存器访问。
///
/// 持有此类型即证明底层物理地址区域已被 identity-map 到内核页表。
/// 不可 Clone（一个映射只有一个 owner），可通过 `&self` 共享读取。
/// Drop 时自动 unmap（由内部 `MappedPages` 负责，除非标记为永久映射）。
pub struct MmioRegion {
    mapping: MappedPages,
}

impl MmioRegion {
    /// 映射 MMIO 区域到指定页表并返回 `MmioRegion`。
    ///
    /// 将 `[paddr, paddr+size)` identity-map 进指定页表。
    /// 用于需要映射到非内核页表的场景（如未来的用户态 MMIO / VFIO）。
    ///
    /// # Errors
    ///
    /// 映射失败时返回错误。
    pub fn map_to(
        pt_ref: &'static SpinLock<PageTable>,
        paddr: PhysAddr,
        size: usize,
    ) -> Result<Self, MemoryError> {
        let mut guard = pt_ref.lock();
        let pa_aligned = paddr.align_down();
        let end = paddr + size;
        guard.identity_map_range(pa_aligned, end, PteFlags::kernel_device())?;
        let page_count = (end.align_up().as_usize() - pa_aligned.as_usize()) / config::PAGE_SIZE;
        let mapping = MappedPages::new_borrowed(
            pt_ref,
            address::VirtAddr::new(pa_aligned.as_usize()),
            page_count,
            PteFlags::kernel_device(),
        );
        drop(guard);
        crate::tlb::flush_tlb();
        Ok(Self { mapping })
    }

    /// 映射 MMIO 区域并返回 `MmioRegion`。
    ///
    /// 将 `[paddr, paddr+size)` identity-map 进内核页表。
    ///
    /// # Errors
    ///
    /// 内核页表未初始化或映射失败时返回错误。
    pub fn map(paddr: PhysAddr, size: usize) -> Result<Self, MemoryError> {
        let kpt = crate::kernel_page_table().ok_or(MemoryError::InvalidPageTable)?;
        Self::map_to(kpt, paddr, size)
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

    /// 读取指定偏移处的寄存器值。
    ///
    /// # Safety
    ///
    /// 调用方必须确保：
    /// 1. `offset + size_of::<T>() <= self.size()`
    /// 2. 偏移对齐到 `T` 的自然对齐边界
    /// 3. 该偏移处的寄存器确实存在且可读
    #[inline]
    pub unsafe fn read_reg<T: zerocopy::FromBytes>(&self, offset: usize) -> T {
        assert!(
            offset + core::mem::size_of::<T>() <= self.size(),
            "MmioRegion::read_reg: offset {:#x} + {} 超出区域大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size(),
        );
        let addr = self.mapping.vaddr().as_usize() + offset;
        assert!(
            addr.is_multiple_of(core::mem::align_of::<T>()),
            "MmioRegion::read_reg: 地址 {:#x} 未对齐到 {} 字节",
            addr,
            core::mem::align_of::<T>(),
        );
        let ptr: *const T = addr as *const T;
        // SAFETY: 调用方保证偏移有效，volatile 防止编译器优化
        unsafe { core::ptr::read_volatile(ptr) }
    }

    /// 写入指定偏移处的寄存器值。
    ///
    /// # Safety
    ///
    /// 调用方必须确保：
    /// 1. `offset + size_of::<T>() <= self.size()`
    /// 2. 偏移对齐到 `T` 的自然对齐边界
    /// 3. 该偏移处的寄存器确实存在且可写
    #[inline]
    pub unsafe fn write_reg<T: zerocopy::IntoBytes>(&self, offset: usize, val: T) {
        assert!(
            offset + core::mem::size_of::<T>() <= self.size(),
            "MmioRegion::write_reg: offset {:#x} + {} 超出区域大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size(),
        );
        let addr = self.mapping.vaddr().as_usize() + offset;
        assert!(
            addr.is_multiple_of(core::mem::align_of::<T>()),
            "MmioRegion::write_reg: 地址 {:#x} 未对齐到 {} 字节",
            addr,
            core::mem::align_of::<T>(),
        );
        let ptr: *mut T = addr as *mut T;
        // SAFETY: 调用方保证偏移有效，volatile 防止编译器优化
        unsafe { core::ptr::write_volatile(ptr, val) }
    }
}

impl core::fmt::Debug for MmioRegion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MmioRegion({}, size={:#x})",
            self.mapping.vaddr(),
            self.size(),
        )
    }
}
