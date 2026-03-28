//! RISC-V Sv39/Sv48/Sv57 PTE 编码。
//!
//! PTE 格式（所有 Sv 模式共用）：
//! - bits [9:0]：flags（V/R/W/X/U/G/A/D + RSW）
//! - bits [53:10]：PPN（物理页号）
//! - bits [63:54]：保留

use super::{PageFlags, PageTableEntry};
use crate::address::PhysAddr;

const PAGE_SHIFT: u32 = config::PAGE_SIZE.trailing_zeros();

/// flags 位宽（RISC-V PTE 格式固定 10 位：bits [9:0]）
const FLAGS_BITS: u32 = 10;

/// PPN 掩码：bits [53:10]
const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00;

impl PageTableEntry {
    /// 从物理地址和标志构造 PTE。
    #[inline]
    pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
        let ppn = ((paddr.as_usize() as u64) >> PAGE_SHIFT) << FLAGS_BITS;
        Self(ppn | flags.bits())
    }

    /// 从 PTE 提取物理地址。
    #[inline]
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((((self.0 & PPN_MASK) >> FLAGS_BITS) << PAGE_SHIFT) as usize)
    }

    /// 从 PTE 提取标志位。
    #[inline]
    pub fn flags(self) -> PageFlags {
        PageFlags::from_bits_truncate(self.0 & 0xFF)
    }

    /// PTE 是否有效（V 位）。
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 & PageFlags::VALID.bits() != 0
    }

    /// 是否为叶节点（R/W/X 至少有一个设置）。
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.0 & (PageFlags::READ | PageFlags::WRITE | PageFlags::EXECUTE).bits() != 0
    }

    /// 空 PTE（全零）。
    #[inline]
    pub fn empty() -> Self {
        Self(0)
    }

    /// 中间节点 PTE（仅 V 位，指向下一级页表）。
    #[inline]
    pub fn new_intermediate(paddr: PhysAddr) -> Self {
        Self::new(paddr, PageFlags::VALID)
    }
}
