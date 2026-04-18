//! 类型化 MMIO 区域——强制 volatile 语义的映射包装。
//!
//! `MmioRegion` 使用 volatile 语义，适合设备寄存器——编译器不会优化掉
//! 对同一地址的重复读写，也不会重排 MMIO 操作。
//!
//! MMIO 地址是硬件寄存器，不是 RAM，不在帧分配器中。
//! 直接使用 PageTable 方法建立 identity mapping。
//! 映射永久存在——不自动 unmap。

use crate::{PteFlags, PteFlagsOps};
use memory_types::PhysAddr;

/// 已映射的 MMIO 区域——提供类型安全的 volatile 寄存器访问。
///
/// MMIO 地址是硬件寄存器，不是 RAM，不在帧分配器中。
/// 直接使用 PageTable 方法建立 identity mapping（VA == PA）。
/// 映射永久存在——不自动 unmap。
pub struct MmioRegion {
    base: memory_types::VirtAddr,
    size: usize,
}

impl MmioRegion {
    /// 将 `[paddr, paddr+size)` identity-map，返回 `MmioRegion`。
    ///
    /// **不要直接调用**——请使用 `memory::map_mmio`，后者包含 RAM 重叠校验。
    /// 此方法保留 `pub` 仅因 `memory` crate 需要跨 crate 调用。
    ///
    /// # Panics
    ///
    /// 页表映射冲突或节点 OOM 时 panic（内核 bug）。
    pub fn map(paddr: PhysAddr, size: usize) -> Self {
        let pa_aligned = paddr.align_down();
        let end_aligned = (paddr + size).align_up();
        let mapped_size = end_aligned.as_usize() - pa_aligned.as_usize();
        let va = memory_types::VirtAddr::new(pa_aligned.as_usize());

        let pt = crate::kernel_page_table();
        pt.identity_map_range(pa_aligned, end_aligned, PteFlags::kernel_device());

        Self {
            base: va,
            size: mapped_size,
        }
    }

    /// 返回 MMIO 区域的基地址。
    #[must_use]
    pub fn base(&self) -> memory_types::VirtAddr {
        self.base
    }

    /// 返回 MMIO 区域的大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// 读取指定偏移处的寄存器值（volatile 语义）。
    #[inline]
    pub fn read_reg<T: zerocopy::FromBytes>(&self, offset: usize) -> T {
        let ptr: *const T =
            check_bounds_and_align::<T>(self.base.as_usize(), self.size, offset, "read_reg");
        // SAFETY: MmioRegion 构造保证地址区间已映射为 device memory；
        // check_bounds_and_align 验证了越界和对齐；FromBytes 保证任意位模式合法
        unsafe { core::ptr::read_volatile(ptr) }
    }

    /// 写入指定偏移处的寄存器值（volatile 语义）。
    #[inline]
    pub fn write_reg<T: zerocopy::IntoBytes>(&self, offset: usize, val: T) {
        let ptr: *const T =
            check_bounds_and_align::<T>(self.base.as_usize(), self.size, offset, "write_reg");
        // SAFETY: MmioRegion 构造保证地址区间已映射为 device memory
        unsafe { core::ptr::write_volatile(ptr as *mut T, val) }
    }
}

/// 验证偏移在范围内且地址对齐到 `T` 的自然边界，返回目标指针。
fn check_bounds_and_align<T>(base: usize, size: usize, offset: usize, fn_name: &str) -> *const T {
    let type_size = core::mem::size_of::<T>();
    assert!(
        type_size > 0,
        "MmioRegion::{fn_name}: 不支持 ZST（size_of::<T>() == 0）"
    );
    assert!(
        type_size <= size && offset <= size - type_size,
        "MmioRegion::{fn_name}: offset {:#x} + {type_size} 超出大小 {:#x}",
        offset,
        size,
    );
    let addr = base + offset;
    let align = core::mem::align_of::<T>();
    assert!(
        addr.is_multiple_of(align),
        "MmioRegion::{fn_name}: 地址 {:#x} 未对齐到 {align} 字节",
        addr,
    );
    addr as *const T
}

impl core::fmt::Debug for MmioRegion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MmioRegion({}, size={:#x})", self.base, self.size)
    }
}
