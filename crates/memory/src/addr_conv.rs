//! 物理-虚拟地址转换。
//!
//! 偏移量由 [`config::PHYS_OFFSET`] 控制，切换到 higher-half kernel 时
//! 修改该常量即可。

use address::{PhysAddr, VirtAddr};

/// 物理地址转虚拟地址。
#[inline]
pub fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    VirtAddr::new(pa.as_usize().wrapping_add(config::PHYS_OFFSET))
}

/// 虚拟地址转物理地址。
#[inline]
pub fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    PhysAddr::new(va.as_usize().wrapping_sub(config::PHYS_OFFSET))
}
