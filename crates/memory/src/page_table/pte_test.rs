//! 宿主机测试用 PTE 编码——与 RISC-V Sv39 相同的位布局。

use super::{PageFlags, PageTableEntry};
use crate::address::PhysAddr;

/// Sv39 PPN 掩码：bits [53:10]
const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00;

impl PageTableEntry {
    #[inline]
    pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
        let ppn = ((paddr.as_usize() as u64) >> 12) << 10;
        Self(ppn | flags.bits())
    }
    #[inline]
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((((self.0 & PPN_MASK) >> 10) << 12) as usize)
    }
    #[inline]
    pub fn flags(self) -> PageFlags {
        PageFlags::from_bits_truncate(self.0 & 0xFF)
    }
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 & PageFlags::VALID.bits() != 0
    }
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.0 & (PageFlags::READ | PageFlags::WRITE | PageFlags::EXECUTE).bits() != 0
    }
    #[inline]
    pub fn empty() -> Self {
        Self(0)
    }
    #[inline]
    pub fn new_intermediate(paddr: PhysAddr) -> Self {
        Self::new(paddr, PageFlags::VALID)
    }
}
