//! 类型化 MMIO 区域——借鉴 Theseus OS 的 `MappedPages::as_type` 模式。
//!
//! `MmioRegion` 封装了一段已映射的 MMIO 地址区域，提供类型安全的
//! 寄存器读写方法，替代裸指针 + `core::ptr::read_volatile` 的传统做法。
//!
//! # 设计理念
//!
//! Theseus OS 通过 `MappedPages` 将所有内存访问绑定到映射的生命周期，
//! 编译器保证不会发生 use-after-unmap。`MmioRegion` 采用类似思路：
//! - `map()` 返回 owned `MmioRegion`（当前不支持 unmap，内核 MMIO 永久有效）
//! - 寄存器访问通过 `read_reg` / `write_reg` 方法，带边界检查
//! - 类型参数确保正确的寄存器宽度（u8/u16/u32/u64）
//!
//! # 与裸指针方式的对比
//!
//! ```ignore
//! // 之前（裸指针）
//! let addr = (base + offset) as *mut u32;
//! unsafe { core::ptr::write_volatile(addr, val) };
//!
//! // 之后（MmioRegion）
//! let region = MmioRegion::map(paddr, size)?;
//! unsafe { region.write_reg::<u32>(offset, val) };
//! ```

#[cfg(not(test))]
use crate::error::KResult;
#[cfg(not(test))]
use crate::memory::address::{PhysAddr, VirtAddr};

/// 已映射的 MMIO 区域——提供类型安全的寄存器访问。
///
/// 持有此类型即证明底层物理地址区域已被 identity-map 到内核页表。
/// 不可 Clone（一个映射只有一个 owner），可通过 `&self` 共享读取。
#[cfg(not(test))]
pub struct MmioRegion {
    base: VirtAddr,
    size: usize,
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
    pub fn map(paddr: PhysAddr, size: usize) -> KResult<Self> {
        let va = super::map_mmio(paddr, size)?;
        Ok(Self { base: va, size })
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
    /// 1. `offset + size_of::<T>() <= self.size`（由 debug_assert 检查）
    /// 2. 偏移对齐到 `T` 的自然对齐边界
    /// 3. 该偏移处的寄存器确实存在且可读
    #[inline]
    pub unsafe fn read_reg<T: Copy>(&self, offset: usize) -> T {
        debug_assert!(
            offset + core::mem::size_of::<T>() <= self.size,
            "MmioRegion::read_reg: offset {:#x} + {} 超出区域大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size,
        );
        let addr = (self.base.as_usize() + offset) as *const T;
        // SAFETY: 调用方保证偏移有效，volatile 防止编译器优化
        unsafe { core::ptr::read_volatile(addr) }
    }

    /// 写入指定偏移处的寄存器值。
    ///
    /// # Safety
    ///
    /// 调用方必须确保：
    /// 1. `offset + size_of::<T>() <= self.size`（由 debug_assert 检查）
    /// 2. 偏移对齐到 `T` 的自然对齐边界
    /// 3. 该偏移处的寄存器确实存在且可写
    #[inline]
    pub unsafe fn write_reg<T: Copy>(&self, offset: usize, val: T) {
        debug_assert!(
            offset + core::mem::size_of::<T>() <= self.size,
            "MmioRegion::write_reg: offset {:#x} + {} 超出区域大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size,
        );
        let addr = (self.base.as_usize() + offset) as *mut T;
        // SAFETY: 调用方保证偏移有效，volatile 防止编译器优化
        unsafe { core::ptr::write_volatile(addr, val) }
    }
}

#[cfg(not(test))]
impl core::fmt::Debug for MmioRegion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MmioRegion({}, size={:#x})", self.base, self.size)
    }
}
