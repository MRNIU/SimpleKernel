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

/// 页大小 shift（4KB 页 = 12），与 `config::PAGE_SIZE_BITS` 同源。
const PAGE_SHIFT: u32 = config::PAGE_SIZE_BITS as u32;

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
    }
}

impl PteFlags {
    /// 内核叶 PTE 的共享基础位：VALID | GLOBAL | ACCESSED。
    ///
    /// SAS 架构下所有内核权限组合共享这三位，各 factory 只需
    /// 在此基础上补 R/W/X/D 即可。
    const KERNEL_BASE: Self = Self::VALID.union(Self::GLOBAL).union(Self::ACCESSED);
}

impl PteFlagsOps for PteFlags {
    #[inline]
    fn kernel_rw() -> Self {
        Self::KERNEL_BASE | Self::READ | Self::WRITE | Self::DIRTY
    }

    #[inline]
    fn kernel_rx() -> Self {
        Self::KERNEL_BASE | Self::READ | Self::EXECUTE
    }

    #[inline]
    fn kernel_ro() -> Self {
        Self::KERNEL_BASE | Self::READ
    }

    /// 固件保留区映射。
    ///
    /// 当前固件保留区使用背景层读写权限，不向上层暴露通用 RWX preset。
    #[inline]
    fn kernel_firmware() -> Self {
        Self::kernel_rw()
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
            paddr.is_aligned(),
            "PageTableEntry::new: 物理地址未页对齐: {paddr}"
        );
        assert!(
            !flags.contains(PteFlags::WRITE) || flags.contains(PteFlags::READ),
            "RISC-V PTE::new: flags={flags:?} 违反 W=1,R=0 禁用组合（Priv Spec §5.4）"
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

    /// 中间节点 PTE（仅 V 位，指向下一级页表）。
    #[inline]
    fn new_intermediate(paddr: PhysAddr) -> Self {
        assert!(
            paddr.is_aligned(),
            "PageTableEntry::new_intermediate: 物理地址未页对齐: {paddr}"
        );
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
