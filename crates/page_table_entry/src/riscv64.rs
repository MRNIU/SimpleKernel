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
        /// 软件定义位——RSW bit 8，标记帧所有权。
        ///
        /// EXCLUSIVE = 1：unmap 时帧归还分配器。
        /// EXCLUSIVE = 0：unmap 时不回收帧（identity map / 共享映射）。
        const EXCLUSIVE = 1 << 8;
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
    fn is_exclusive(self) -> bool {
        self.contains(Self::EXCLUSIVE)
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
    fn with_exclusive(self) -> Self {
        self | Self::EXCLUSIVE
    }

    #[inline]
    fn without_exclusive(self) -> Self {
        self.difference(Self::EXCLUSIVE)
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
        debug_assert!(
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个 PteFlags 单独编解码往返（WRITE 需搭配 READ，因 W=1 R=0 非法）。
    #[test]
    fn each_flag_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let all_flags = [
            PteFlags::VALID,
            PteFlags::READ,
            PteFlags::READ | PteFlags::WRITE,
            PteFlags::EXECUTE,
            PteFlags::USER,
            PteFlags::GLOBAL,
            PteFlags::ACCESSED,
            PteFlags::DIRTY,
            PteFlags::EXCLUSIVE,
        ];
        for &flag in &all_flags {
            let pte = PageTableEntry::new(pa, flag);
            assert_eq!(pte.flags(), flag, "标志 {:?} 编解码往返失败", flag);
        }
    }

    /// W^X 安全不变量：数据页不可执行，代码页不可写。
    #[test]
    fn wx_invariants() {
        let rw = PteFlags::kernel_rw();
        assert!(rw.is_writable());
        assert!(!rw.contains(PteFlags::EXECUTE));

        let rx = PteFlags::kernel_rx();
        assert!(!rx.is_writable());
        assert!(rx.contains(PteFlags::EXECUTE));

        let ro = PteFlags::kernel_ro();
        assert!(!ro.is_writable());
        assert!(!ro.contains(PteFlags::EXECUTE));

        let rwx = PteFlags::kernel_rwx();
        assert!(rwx.is_writable());
        assert!(rwx.contains(PteFlags::EXECUTE));

        let dev = PteFlags::kernel_device();
        assert!(!dev.contains(PteFlags::EXECUTE));
    }

    /// for_leaf_at_level 不改变标志位（格式与层级无关）。
    #[test]
    fn for_leaf_at_level_is_identity() {
        let flags = PteFlags::kernel_rw();
        assert_eq!(flags.for_leaf_at_level(0), flags);
        assert_eq!(flags.for_leaf_at_level(1), flags);
        assert_eq!(flags.for_leaf_at_level(2), flags);
    }

    /// EXCLUSIVE 软件位编解码往返。
    #[test]
    fn exclusive_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PteFlags::kernel_rw().with_exclusive();
        let pte = PageTableEntry::new(pa, flags);
        assert!(pte.flags().is_exclusive());
        assert_eq!(pte.paddr(), pa);

        let pte_no_excl = PageTableEntry::new(pa, PteFlags::kernel_rw());
        assert!(!pte_no_excl.flags().is_exclusive());
    }

    /// 用户态 preset 设置了 USER 位且未设 GLOBAL。
    #[test]
    fn user_presets_have_user_bit() {
        let rw = PteFlags::user_rw();
        assert!(rw.is_user());
        assert!(!rw.contains(PteFlags::GLOBAL));

        let rx = PteFlags::user_rx();
        assert!(rx.is_user());
        assert!(!rx.contains(PteFlags::GLOBAL));

        let ro = PteFlags::user_ro();
        assert!(ro.is_user());
        assert!(!ro.contains(PteFlags::GLOBAL));

        let rwx = PteFlags::user_rwx();
        assert!(rwx.is_user());
        assert!(!rwx.contains(PteFlags::GLOBAL));
    }

    /// 用户态 preset 的 W^X 不变量。
    #[test]
    fn user_wx_invariants() {
        let rw = PteFlags::user_rw();
        assert!(rw.is_writable());
        assert!(!rw.contains(PteFlags::EXECUTE));

        let rx = PteFlags::user_rx();
        assert!(!rx.is_writable());
        assert!(rx.contains(PteFlags::EXECUTE));

        let ro = PteFlags::user_ro();
        assert!(!ro.is_writable());
        assert!(!ro.contains(PteFlags::EXECUTE));

        let rwx = PteFlags::user_rwx();
        assert!(rwx.is_writable());
        assert!(rwx.contains(PteFlags::EXECUTE));
    }

    /// V=0 的 PTE 不应被视为叶节点。
    #[test]
    fn invalid_pte_is_not_leaf() {
        let pte = PageTableEntry::from_raw(PteFlags::READ.bits());
        assert!(!pte.is_valid());
        assert!(!pte.is_leaf(0));
    }

    /// 中间节点 PTE（V=1，无 R/W/X）保留地址且不是叶节点。
    #[test]
    fn intermediate_preserves_addr_and_is_not_leaf() {
        let pa = PhysAddr::new(0x8020_0000);
        let pte = PageTableEntry::new_intermediate(pa);
        assert!(pte.is_valid());
        assert_eq!(pte.paddr(), pa);
        assert!(!pte.is_leaf(0));
        assert!(!pte.is_leaf(1));
    }
}
