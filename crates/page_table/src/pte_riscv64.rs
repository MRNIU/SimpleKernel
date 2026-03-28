//! RISC-V Sv39/Sv48/Sv57 PTE 编码。
//!
//! PTE 格式（所有 Sv 模式共用）：
//! - bits [9:0]：flags（V/R/W/X/U/G/A/D + RSW）
//! - bits [53:10]：PPN（物理页号）
//! - bits [63:54]：保留

use bitflags::bitflags;

use super::{PageTableEntry, PteFlagsOps, PteOps};
use address::PhysAddr;

const PAGE_SHIFT: u32 = config::PAGE_SIZE.trailing_zeros();

/// flags 位宽（RISC-V PTE 格式固定 10 位：bits [9:0]）
const FLAGS_BITS: u32 = 10;

/// PPN 掩码：bits [53:10]
const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00;

bitflags! {
    /// RISC-V Sv39/Sv48/Sv57 页表项标志位（硬件原生位位置）。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PteFlags: u64 {
        const VALID    = 1 << 0;
        const READ     = 1 << 1;
        const WRITE    = 1 << 2;
        const EXECUTE  = 1 << 3;
        const USER     = 1 << 4;
        const GLOBAL   = 1 << 5;
        const ACCESSED = 1 << 6;
        const DIRTY    = 1 << 7;
        /// 软件定义位——RSW bit 8，标记帧所有权（参考 Theseus EXCLUSIVE）。
        ///
        /// EXCLUSIVE = 1：unmap 时帧归还分配器。
        /// EXCLUSIVE = 0：unmap 时不回收帧（identity map / 共享映射）。
        const EXCLUSIVE = 1 << 8;
    }
}

impl PteFlagsOps for PteFlags {
    /// 内核读写数据映射 (V | R | W | G | A | D)。
    #[inline]
    fn kernel_rw() -> Self {
        Self::VALID | Self::READ | Self::WRITE | Self::GLOBAL | Self::ACCESSED | Self::DIRTY
    }

    /// 内核读-执行映射 (V | R | X | G | A)。
    #[inline]
    fn kernel_rx() -> Self {
        Self::VALID | Self::READ | Self::EXECUTE | Self::GLOBAL | Self::ACCESSED
    }

    /// 内核只读映射 (V | R | G | A)。
    #[inline]
    fn kernel_ro() -> Self {
        Self::VALID | Self::READ | Self::GLOBAL | Self::ACCESSED
    }

    /// 内核读写执行映射 (V | R | W | X | G | A | D)。
    #[inline]
    fn kernel_rwx() -> Self {
        Self::VALID
            | Self::READ
            | Self::WRITE
            | Self::EXECUTE
            | Self::GLOBAL
            | Self::ACCESSED
            | Self::DIRTY
    }

    /// 设备 MMIO 映射。
    ///
    /// RISC-V 没有页表级缓存属性控制（由 PMA/Svpbmt 扩展管理），
    /// 当前与 `kernel_rw()` 相同。
    #[inline]
    fn kernel_device() -> Self {
        Self::kernel_rw()
    }

    /// 是否具有写权限。
    #[inline]
    fn is_writable(self) -> bool {
        self.contains(Self::WRITE)
    }

    /// RISC-V 的 PTE 格式与层级无关——叶节点仅由 R/W/X 位区分，
    /// 无需为不同层级调整标志位，直接返回 self。
    #[inline]
    fn for_leaf_at_level(self, _level: usize) -> Self {
        self
    }

    #[inline]
    fn is_exclusive(self) -> bool {
        self.contains(Self::EXCLUSIVE)
    }

    #[inline]
    fn with_exclusive(self) -> Self {
        self | Self::EXCLUSIVE
    }
}

impl PteOps for PageTableEntry {
    type Flags = PteFlags;

    /// 从物理地址和标志构造 PTE。
    #[inline]
    fn new(paddr: PhysAddr, flags: PteFlags) -> Self {
        let ppn = ((paddr.as_usize() as u64) >> PAGE_SHIFT) << FLAGS_BITS;
        Self(ppn | flags.bits())
    }

    /// 从 PTE 提取物理地址。
    #[inline]
    fn paddr(self) -> PhysAddr {
        PhysAddr::new((((self.0 & PPN_MASK) >> FLAGS_BITS) << PAGE_SHIFT) as usize)
    }

    /// 从 PTE 提取标志位。
    #[inline]
    fn flags(self) -> PteFlags {
        PteFlags::from_bits_truncate(self.0 & ((1 << FLAGS_BITS) - 1))
    }

    /// PTE 是否有效（V 位）。
    #[inline]
    fn is_valid(self) -> bool {
        self.0 & PteFlags::VALID.bits() != 0
    }

    /// 是否为叶节点。
    ///
    /// RISC-V 规范：R/W/X 至少有一个设置即为叶节点，与层级无关。
    #[inline]
    fn is_leaf(self, _level: usize) -> bool {
        self.0 & (PteFlags::READ | PteFlags::WRITE | PteFlags::EXECUTE).bits() != 0
    }

    /// 空 PTE（全零）。
    #[inline]
    fn empty() -> Self {
        Self(0)
    }

    /// 中间节点 PTE（仅 V 位，指向下一级页表）。
    #[inline]
    fn new_intermediate(paddr: PhysAddr) -> Self {
        Self::new(paddr, PteFlags::VALID)
    }
}
