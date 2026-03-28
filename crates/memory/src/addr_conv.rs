//! 地址转换与映射工具。

use address::{PhysAddr, VirtAddr};

use crate::page_table::PteFlags;

/// 物理地址转虚拟地址（当前为 identity mapping，直接透传）。
pub fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    VirtAddr::new(pa.as_usize())
}

/// 虚拟地址转物理地址（当前为 identity mapping，直接透传）。
pub fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    PhysAddr::new(va.as_usize())
}

/// 将 `[start, end)` 物理地址区间 identity-map 到页表中。
///
/// 自动使用最大可用页大小（1GB / 2MB / 4KB），减少 TLB 压力和页表内存占用。
/// 地址和剩余大小都对齐到大页边界时才使用大页映射。
///
/// # Errors
///
/// 映射冲突或 `start >= end` 时返回错误。
pub fn identity_map_range(
    pt: &mut crate::page_table::PageTable,
    start: PhysAddr,
    end: PhysAddr,
    flags: PteFlags,
) -> Result<(), crate::error::MemoryError> {
    let mut addr = start.align_down();
    let end_aligned = end.align_up();

    if addr.as_usize() >= end_aligned.as_usize() {
        return Err(crate::error::MemoryError::MapFailed);
    }

    while addr.as_usize() < end_aligned.as_usize() {
        let remaining = end_aligned.as_usize() - addr.as_usize();
        let va = VirtAddr::new(addr.as_usize());

        // 从最大页尝试到最小页
        let mut mapped = false;
        for level in (1..config::PT_LEVELS).rev() {
            let page_size = crate::page_table::page_size_at_level(level);
            if addr.as_usize().is_multiple_of(page_size) && remaining >= page_size {
                pt.map_at_level(va, addr, flags, level)?;
                addr += page_size;
                mapped = true;
                break;
            }
        }
        if !mapped {
            pt.map_page(va, addr, flags)?;
            addr += config::PAGE_SIZE;
        }
    }
    Ok(())
}
