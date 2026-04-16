//! RISC-V Sv39/Sv48/Sv57 PTE 编码。
//!
//! PTE 格式（所有 Sv 模式共用）：
//! - bits [9:0]：flags（V/R/W/X/U/G/A/D + RSW）
//! - bits [53:10]：PPN（物理页号）
//! - bits [63:54]：保留
//!
//! [RISC-V Privileged Spec §5.4](https://github.com/riscv/riscv-isa-manual/releases)

use bitflags::bitflags;

use crate::{PteFlagsOps, PteOps};
use memory_types::PhysAddr;

/// 4KB 页：PAGE_SHIFT = 12
const PAGE_SHIFT: u32 = 12;

/// flags 位宽（RISC-V PTE 格式固定 10 位：bits [9:0]）
const FLAGS_BITS: u32 = 10;

/// PPN 掩码：bits [53:10]
const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00;

/// RISC-V 页表项（64 位）。
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl core::fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PageTableEntry")
            .field(&format_args!("{:#018x}", self.0))
            .finish()
    }
}

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
        /// 软件保留位 RSW[0]——MappedPages 所有权标记。
        /// 硬件忽略此位（RISC-V Privileged Spec §5.4）。
        const CLAIMED  = 1 << 8;
    }
}

impl PteFlagsOps for PteFlags {
    #[inline]
    fn kernel_rw() -> Self {
        Self::VALID | Self::READ | Self::WRITE | Self::GLOBAL | Self::ACCESSED | Self::DIRTY
    }

    #[inline]
    fn kernel_rx() -> Self {
        Self::VALID | Self::READ | Self::EXECUTE | Self::GLOBAL | Self::ACCESSED
    }

    #[inline]
    fn kernel_ro() -> Self {
        Self::VALID | Self::READ | Self::GLOBAL | Self::ACCESSED
    }

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
    // TODO(svpbmt): 当平台支持 Svpbmt 扩展时，需使用 PBMT 位设置
    // NC（Non-Cacheable）或 IO 属性，避免 MMIO 区域被 CPU 缓存。
    #[inline]
    fn kernel_device() -> Self {
        Self::kernel_rw()
    }

    /// 用户态读写数据映射（不可执行）。
    ///
    /// 设置 USER 位使页面仅在 U-mode 可访问；不设 GLOBAL，
    /// 因为用户页面是 per-process 的（配合 ASID 使用）。
    #[inline]
    fn user_rw() -> Self {
        Self::VALID | Self::READ | Self::WRITE | Self::USER | Self::ACCESSED | Self::DIRTY
    }

    /// 用户态读-执行映射（不可写）。
    #[inline]
    fn user_rx() -> Self {
        Self::VALID | Self::READ | Self::EXECUTE | Self::USER | Self::ACCESSED
    }

    /// 用户态只读映射。
    #[inline]
    fn user_ro() -> Self {
        Self::VALID | Self::READ | Self::USER | Self::ACCESSED
    }

    /// 用户态读写执行映射。
    #[inline]
    fn user_rwx() -> Self {
        Self::VALID
            | Self::READ
            | Self::WRITE
            | Self::EXECUTE
            | Self::USER
            | Self::ACCESSED
            | Self::DIRTY
    }

    #[inline]
    fn is_readable(self) -> bool {
        self.contains(Self::READ)
    }

    #[inline]
    fn is_writable(self) -> bool {
        self.contains(Self::WRITE)
    }

    #[inline]
    fn is_executable(self) -> bool {
        self.contains(Self::EXECUTE)
    }

    #[inline]
    fn is_user(self) -> bool {
        self.contains(Self::USER)
    }

    #[inline]
    fn is_accessed(self) -> bool {
        self.contains(Self::ACCESSED)
    }

    #[inline]
    fn is_dirty(self) -> bool {
        self.contains(Self::DIRTY)
    }

    #[inline]
    fn is_claimed(self) -> bool {
        self.contains(Self::CLAIMED)
    }

    #[inline]
    fn with_writable(self, w: bool) -> Self {
        if w {
            self | Self::WRITE | Self::DIRTY
        } else {
            self.difference(Self::WRITE | Self::DIRTY)
        }
    }

    #[inline]
    fn with_executable(self, x: bool) -> Self {
        if x {
            self | Self::EXECUTE
        } else {
            self.difference(Self::EXECUTE)
        }
    }

    #[inline]
    fn with_claimed(self, claimed: bool) -> Self {
        if claimed {
            self | Self::CLAIMED
        } else {
            self.difference(Self::CLAIMED)
        }
    }

    /// RISC-V 的 PTE 格式与层级无关——叶节点仅由 R/W/X 位区分，
    /// 无需为不同层级调整标志位，直接返回 self。
    #[inline]
    fn for_leaf_at_level(self, _level: usize) -> Self {
        self
    }
}

impl PteOps for PageTableEntry {
    type Flags = PteFlags;

    #[inline]
    fn new(paddr: PhysAddr, flags: PteFlags) -> Self {
        assert!(
            !flags.contains(PteFlags::WRITE) || flags.contains(PteFlags::READ),
            "RISC-V spec 禁止 W=1, R=0 的标志组合"
        );
        let ppn = ((paddr.as_usize() as u64) >> PAGE_SHIFT) << FLAGS_BITS;
        Self(ppn | flags.bits())
    }

    #[inline]
    fn paddr(self) -> PhysAddr {
        PhysAddr::new((((self.0 & PPN_MASK) >> FLAGS_BITS) << PAGE_SHIFT) as usize)
    }

    #[inline]
    fn flags(self) -> PteFlags {
        PteFlags::from_bits_truncate(self.0 & ((1 << FLAGS_BITS) - 1))
    }

    #[inline]
    fn is_valid(self) -> bool {
        self.0 & PteFlags::VALID.bits() != 0
    }

    /// RISC-V 规范：V=1 且 R/W/X 至少有一个设置即为叶节点，与层级无关。
    /// V=0 的 PTE 无效，不视为叶。
    #[inline]
    fn is_leaf(self, _level: usize) -> bool {
        self.is_valid()
            && self.0 & (PteFlags::READ | PteFlags::WRITE | PteFlags::EXECUTE).bits() != 0
    }

    #[inline]
    fn empty() -> Self {
        Self(0)
    }

    /// 中间节点 PTE（仅 V 位，指向下一级页表）。
    #[inline]
    fn new_intermediate(paddr: PhysAddr) -> Self {
        let ppn = ((paddr.as_usize() as u64) >> PAGE_SHIFT) << FLAGS_BITS;
        Self(ppn | PteFlags::VALID.bits())
    }

    #[inline]
    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    #[inline]
    fn as_raw(self) -> u64 {
        self.0
    }
}
