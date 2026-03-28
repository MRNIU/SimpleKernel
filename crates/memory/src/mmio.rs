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

#[cfg(not(test))]
use crate::address::{PhysAddr, VirtAddr};
#[cfg(not(test))]
use crate::error::MemoryError;

/// 已映射的 MMIO 区域——提供类型安全的寄存器访问。
///
/// 持有此类型即证明底层物理地址区域已被 identity-map 到内核页表。
/// 不可 Clone（一个映射只有一个 owner），可通过 `&self` 共享读取。
/// Drop 时自动 unmap（除非标记为永久映射）。
#[cfg(not(test))]
pub struct MmioRegion {
    base: VirtAddr,
    size: usize,
    permanent: bool,
}

#[cfg(not(test))]
impl MmioRegion {
    /// 映射 MMIO 区域并返回 `MmioRegion`。
    ///
    /// 将 `[paddr, paddr+size)` identity-map 进内核页表。
    ///
    /// # Errors
    ///
    /// 内核页表未初始化或映射失败时返回错误。
    pub fn map(paddr: PhysAddr, size: usize) -> Result<Self, MemoryError> {
        let va = super::map_mmio(paddr, size)?;
        Ok(Self {
            base: va,
            size,
            permanent: false,
        })
    }

    /// 标记为永久映射——drop 时不 unmap。
    ///
    /// 用于 PLIC、GIC 等内核生命周期内永远需要的 MMIO 区域。
    #[must_use]
    pub fn into_permanent(mut self) -> Self {
        self.permanent = true;
        self
    }

    /// 返回 MMIO 区域的基地址。
    #[must_use]
    pub fn base(&self) -> VirtAddr {
        self.base
    }

    /// 返回 MMIO 区域的大小。
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// 读取指定偏移处的寄存器值。
    ///
    /// # Safety
    ///
    /// 调用方必须确保：
    /// 1. `offset + size_of::<T>() <= self.size`
    /// 2. 偏移对齐到 `T` 的自然对齐边界
    /// 3. 该偏移处的寄存器确实存在且可读
    #[inline]
    pub unsafe fn read_reg<T: zerocopy::FromBytes>(&self, offset: usize) -> T {
        assert!(
            offset + core::mem::size_of::<T>() <= self.size,
            "MmioRegion::read_reg: offset {:#x} + {} 超出区域大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size,
        );
        let addr: *const T = (self.base + offset).as_ptr();
        // SAFETY: 调用方保证偏移有效，volatile 防止编译器优化
        unsafe { core::ptr::read_volatile(addr) }
    }

    /// 写入指定偏移处的寄存器值。
    ///
    /// # Safety
    ///
    /// 调用方必须确保：
    /// 1. `offset + size_of::<T>() <= self.size`
    /// 2. 偏移对齐到 `T` 的自然对齐边界
    /// 3. 该偏移处的寄存器确实存在且可写
    #[inline]
    pub unsafe fn write_reg<T: zerocopy::IntoBytes>(&self, offset: usize, val: T) {
        assert!(
            offset + core::mem::size_of::<T>() <= self.size,
            "MmioRegion::write_reg: offset {:#x} + {} 超出区域大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size,
        );
        let addr: *mut T = (self.base + offset).as_mut_ptr();
        // SAFETY: 调用方保证偏移有效，volatile 防止编译器优化
        unsafe { core::ptr::write_volatile(addr, val) }
    }
}

#[cfg(not(test))]
impl Drop for MmioRegion {
    fn drop(&mut self) {
        if self.permanent {
            return;
        }
        if let Some(kpt) = crate::kernel_page_table() {
            let mut guard = kpt.lock();
            let start =
                crate::address::VirtAddr::new(self.base.as_usize() & !(config::PAGE_SIZE - 1));
            let end_raw = self.base.as_usize() + self.size;
            let end = (end_raw + config::PAGE_SIZE - 1) & !(config::PAGE_SIZE - 1);
            let mut addr = start;
            while addr.as_usize() < end {
                let _ = guard.unmap_page(addr);
                addr += config::PAGE_SIZE;
            }
            crate::tlb::flush_tlb();
        }
    }
}

#[cfg(not(test))]
impl core::fmt::Debug for MmioRegion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MmioRegion({}, size={:#x}{})",
            self.base,
            self.size,
            if self.permanent { ", permanent" } else { "" }
        )
    }
}
