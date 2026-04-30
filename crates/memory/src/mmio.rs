//! 类型化 MMIO 区域——强制 volatile 语义的映射包装。
//!
//! `MmioRegion` 使用 volatile 语义，适合设备寄存器——编译器不会优化掉
//! 对同一地址的重复读写，也不会重排 MMIO 操作。
//!
//! MMIO 地址是硬件寄存器，不是 RAM，不在帧分配器中。
//! 通过 `paging::kernel_page_table().identity_map_range` 建立 Device 权限的
//! identity 映射，永久存在——不自动 unmap。

use memory_types::{PhysAddr, Span};
use paging::{PteFlags, PteFlagsOps};

use crate::globals::MEMORY_INFO;

/// 已映射的 MMIO 区域——提供类型安全的 volatile 寄存器访问。
pub struct MmioRegion {
    base: memory_types::VirtAddr,
    size: usize,
}

impl MmioRegion {
    /// 将 `[paddr, paddr+size)` identity-map 为 Device 内存，返回 `MmioRegion`。
    ///
    /// RAM 重叠校验通过读取 `MEMORY_INFO` 内部完成，调用方无需显式传入 RAM 范围。
    ///
    /// # Panics
    ///
    /// - `MEMORY_INFO` 未初始化（必须在 `memory::init()` 之后调用）
    /// - `[paddr, paddr+size)` 与 RAM 范围重叠（拒绝将 RAM 重映射为 Device 内存）
    /// - 页表映射冲突或节点 OOM（内核 bug）
    pub fn map(paddr: PhysAddr, size: usize) -> Self {
        let info = MEMORY_INFO
            .get()
            .expect("MmioRegion::map: MEMORY_INFO 未初始化（应在 memory::init 之后调用）");
        let ram = Span::new(
            info.physical_memory_addr,
            info.physical_memory_addr + info.physical_memory_size,
        );
        let req = Span::new(paddr, paddr + size);

        assert!(
            !ram.overlaps(req),
            "MmioRegion::map: paddr {} + {:#x} 与 RAM 范围 [{}, {}) 重叠——拒绝映射为 Device 内存",
            paddr,
            size,
            ram.start(),
            ram.end()
        );

        let pa_aligned = paddr.align_down();
        let end_aligned = (paddr + size).align_up();
        let mapped_size = end_aligned - pa_aligned;

        paging::kernel_page_table().identity_map_range(
            pa_aligned,
            end_aligned,
            PteFlags::kernel_device(),
        );
        tlb::flush_tlb();

        Self {
            base: pa_aligned.to_virt(),
            size: mapped_size,
        }
    }

    #[must_use]
    pub fn base(&self) -> memory_types::VirtAddr {
        self.base
    }

    /// 读取指定偏移处的寄存器值（volatile 语义）。
    #[inline]
    pub fn read_reg<T: zerocopy::FromBytes>(&self, offset: usize) -> T {
        let ptr: *const T = self.reg_ptr::<T>(offset);
        // SAFETY: reg_ptr 已验证越界和对齐；MmioRegion 构造保证地址已映射为 device memory；
        // FromBytes 保证任意位模式合法
        unsafe { core::ptr::read_volatile(ptr) }
    }

    /// 写入指定偏移处的寄存器值（volatile 语义）。
    #[inline]
    pub fn write_reg<T: zerocopy::IntoBytes>(&self, offset: usize, val: T) {
        let ptr: *mut T = self.reg_ptr::<T>(offset) as *mut T;
        // SAFETY: reg_ptr 已验证越界和对齐；MmioRegion 构造保证地址已映射为 device memory
        unsafe { core::ptr::write_volatile(ptr, val) }
    }

    /// 计算偏移处的寄存器指针，校验 `[offset, offset+size_of::<T>())` 在区域内且对齐到 `T`。
    #[inline]
    fn reg_ptr<T>(&self, offset: usize) -> *const T {
        let type_size = core::mem::size_of::<T>();
        assert!(type_size > 0, "MmioRegion: 不支持 ZST");
        assert!(
            type_size <= self.size && offset <= self.size - type_size,
            "MmioRegion: offset {:#x} + {type_size} 超出区域大小 {:#x}",
            offset,
            self.size,
        );
        let addr = self.base.as_usize() + offset;
        let align = core::mem::align_of::<T>();
        assert!(
            addr.is_multiple_of(align),
            "MmioRegion: 地址 {:#x} 未对齐到 {align} 字节",
            addr,
        );
        addr as *const T
    }
}
