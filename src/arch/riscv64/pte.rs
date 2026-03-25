//! RISC-V Sv39 页表项编码

use crate::memory::address::PhysAddr;
use crate::memory::page_table::{PageFlags, PageTableEntry};

/// Sv39 PPN 掩码：bits [53:10]
const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00;

/// 页表层级数（Sv39 = 3 级）
pub const PT_LEVELS: usize = 3;

impl PageTableEntry {
    /// 构造叶 PTE，将 `paddr` 编码为 PPN，附加 `flags`。
    ///
    /// PPN 占据 bits [53:10] → ppn = (paddr >> 12) << 10。
    #[inline]
    pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
        let ppn = ((paddr.as_usize() as u64) >> 12) << 10;
        Self(ppn | flags.bits())
    }

    /// 返回此 PTE 中编码的物理地址。
    #[inline]
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((((self.0 & PPN_MASK) >> 10) << 12) as usize)
    }

    /// 返回标志位（bits [7:0]）。
    #[inline]
    pub fn flags(self) -> PageFlags {
        PageFlags::from_bits_truncate(self.0 & 0xFF)
    }

    /// VALID 位是否设置？
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 & PageFlags::VALID.bits() != 0
    }

    /// 是否为叶节点（R、W 或 X 被设置）？
    ///
    /// Sv39 中间节点的 R=W=X=0。
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.0 & (PageFlags::READ | PageFlags::WRITE | PageFlags::EXECUTE).bits() != 0
    }

    /// 无效（全零）PTE。
    #[inline]
    pub fn empty() -> Self {
        Self(0)
    }

    /// 构造中间节点 PTE（仅设置 VALID，R=W=X=0）。
    #[inline]
    pub fn new_intermediate(paddr: PhysAddr) -> Self {
        Self::new(paddr, PageFlags::VALID)
    }
}
