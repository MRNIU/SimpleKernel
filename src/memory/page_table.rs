//! 多级页表抽象。
//!
//! `PageFlags` 和 `PageTableEntry` 的结构体定义在此处（架构无关）。
//! `PageTableEntry` 的编码实现（Sv39 / ARMv8）在各架构的 `pte.rs` 中。
//! `PageTable`（页表 walk/map/unmap 逻辑）也在此处，通过
//! `PageTableEntry::new_intermediate()` 屏蔽中间节点的架构差异。

use bitflags::bitflags;

use crate::memory::address::PhysAddr;

bitflags! {
    /// 架构无关的页表项标志位。
    ///
    /// 位位置与 RISC-V Sv39 对齐。AArch64 的 `pte.rs` 在构造 PTE 时
    /// 将这些标志翻译为 ARM 描述符位。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PageFlags: u64 {
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

impl PageFlags {
    /// 内核读写数据映射 (V | R | W | G | A | D)。
    #[inline]
    pub fn kernel_rw() -> Self {
        Self::VALID | Self::READ | Self::WRITE | Self::GLOBAL | Self::ACCESSED | Self::DIRTY
    }

    /// 内核读-执行映射 (V | R | X | G | A)。
    #[inline]
    pub fn kernel_rx() -> Self {
        Self::VALID | Self::READ | Self::EXECUTE | Self::GLOBAL | Self::ACCESSED
    }

    /// 内核只读映射 (V | R | G | A)。
    #[inline]
    pub fn kernel_ro() -> Self {
        Self::VALID | Self::READ | Self::GLOBAL | Self::ACCESSED
    }

    /// 内核读写执行映射 (V | R | W | X | G | A | D)。
    #[inline]
    pub fn kernel_rwx() -> Self {
        Self::VALID
            | Self::READ
            | Self::WRITE
            | Self::EXECUTE
            | Self::GLOBAL
            | Self::ACCESSED
            | Self::DIRTY
    }
}

/// 单个硬件页表项（64 位）。
///
/// 方法 `new`、`paddr`、`flags`、`is_valid`、`is_leaf`、`empty`、`new_intermediate`
/// 由各架构的 `pte.rs` 提供。
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(pub(crate) u64);

#[cfg(test)]
impl PageTableEntry {
    const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00;

    #[inline]
    pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
        let ppn = ((paddr.as_usize() as u64) >> 12) << 10;
        Self(ppn | flags.bits())
    }
    #[inline]
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((((self.0 & Self::PPN_MASK) >> 10) << 12) as usize)
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

#[cfg(not(test))]
mod inner {
    use super::{PageFlags, PageTableEntry};
    use crate::config::PAGE_SIZE;
    use crate::error::{ErrorCode, KResult};
    use crate::memory::address::{PhysAddr, VirtAddr};
    use crate::memory::frame::FrameTracker;

    /// 每页 PTE 数量（4KB / 8 = 512）
    pub const ENTRIES_PER_PAGE: usize = PAGE_SIZE / 8;

    /// 页表层级数——由各架构 `pte.rs` 定义
    #[cfg(target_arch = "riscv64")]
    pub const PT_LEVELS: usize = crate::arch::riscv64::pte::PT_LEVELS;
    #[cfg(target_arch = "aarch64")]
    pub const PT_LEVELS: usize = crate::arch::aarch64::pte::PT_LEVELS;

    /// 从虚拟地址中提取第 `level` 级的 9 位 VPN 索引。
    #[inline]
    pub fn vpn_index(va: VirtAddr, level: usize) -> usize {
        (va.as_usize() >> (12 + level * 9)) & 0x1FF
    }

    /// 将物理地址解释为页表项数组。
    ///
    /// # Safety
    /// `paddr` 必须指向有效、页对齐、由 `FrameTracker` 分配的帧，
    /// 且当前不存在其他可变引用。
    unsafe fn pte_array(paddr: PhysAddr) -> &'static mut [PageTableEntry; ENTRIES_PER_PAGE] {
        unsafe { &mut *(paddr.as_usize() as *mut [PageTableEntry; ENTRIES_PER_PAGE]) }
    }

    /// 多级页表。
    ///
    /// 拥有根帧及所有遍历过程中分配的中间帧。
    /// drop 时自动归还所有帧。
    pub struct PageTable {
        root: FrameTracker,
        frames: alloc::vec::Vec<FrameTracker>,
    }

    impl PageTable {
        pub fn new() -> KResult<Self> {
            let root = FrameTracker::alloc()?;
            Ok(Self {
                root,
                frames: alloc::vec::Vec::new(),
            })
        }

        #[inline]
        pub fn root_paddr(&self) -> PhysAddr {
            self.root.paddr()
        }

        /// 遍历到 `va` 对应的叶 PTE，必要时分配中间节点。
        pub fn find_or_create_pte(&mut self, va: VirtAddr) -> KResult<&'static mut PageTableEntry> {
            let mut paddr = self.root.paddr();

            for level in (1..PT_LEVELS).rev() {
                let table = unsafe { pte_array(paddr) };
                let idx = vpn_index(va, level);
                let pte = &mut table[idx];

                if !pte.is_valid() {
                    let frame = FrameTracker::alloc()?;
                    let frame_paddr = frame.paddr();
                    // new_intermediate 由各架构 pte.rs 定义：
                    // riscv64: VALID-only PTE
                    // aarch64: table descriptor
                    *pte = PageTableEntry::new_intermediate(frame_paddr);
                    self.frames.push(frame);
                }

                paddr = pte.paddr();
            }

            let table = unsafe { pte_array(paddr) };
            let idx = vpn_index(va, 0);
            Ok(&mut table[idx])
        }

        /// 只读遍历，不分配。
        pub fn find_pte(&self, va: VirtAddr) -> Option<&'static PageTableEntry> {
            let mut paddr = self.root.paddr();

            for level in (1..PT_LEVELS).rev() {
                let table = unsafe { pte_array(paddr) };
                let idx = vpn_index(va, level);
                let pte = &table[idx];
                if !pte.is_valid() {
                    return None;
                }
                paddr = pte.paddr();
            }

            let table = unsafe { pte_array(paddr) };
            let idx = vpn_index(va, 0);
            Some(&table[idx])
        }

        pub fn map_page(&mut self, va: VirtAddr, pa: PhysAddr, flags: PageFlags) -> KResult<()> {
            let pte = self.find_or_create_pte(va)?;
            if pte.is_valid() {
                return Err(ErrorCode::VmMapFailed);
            }
            *pte = PageTableEntry::new(pa, flags);
            Ok(())
        }

        pub fn unmap_page(&mut self, va: VirtAddr) -> KResult<PhysAddr> {
            let pte = self.find_or_create_pte(va)?;
            if !pte.is_valid() {
                return Err(ErrorCode::VmPageNotMapped);
            }
            let old_pa = pte.paddr();
            *pte = PageTableEntry::empty();
            Ok(old_pa)
        }

        pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PageFlags)> {
            let pte = self.find_pte(va)?;
            if pte.is_valid() && pte.is_leaf() {
                Some((pte.paddr(), pte.flags()))
            } else {
                None
            }
        }
    }
}

#[cfg(not(test))]
pub use inner::PageTable;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pte_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PageFlags::VALID | PageFlags::READ | PageFlags::WRITE;
        let pte = PageTableEntry::new(pa, flags);
        assert_eq!(pte.paddr(), pa, "paddr round-trip failed");
        let recovered = pte.flags();
        assert!(recovered.contains(PageFlags::VALID));
        assert!(recovered.contains(PageFlags::READ));
        assert!(recovered.contains(PageFlags::WRITE));
    }

    #[test]
    fn pte_empty_is_invalid() {
        let pte = PageTableEntry::empty();
        assert!(!pte.is_valid());
        assert!(!pte.is_leaf());
    }

    #[test]
    fn page_flags_presets() {
        let rw = PageFlags::kernel_rw();
        assert!(rw.contains(PageFlags::VALID));
        assert!(rw.contains(PageFlags::READ));
        assert!(rw.contains(PageFlags::WRITE));
        assert!(rw.contains(PageFlags::GLOBAL));
        assert!(rw.contains(PageFlags::ACCESSED));
        assert!(rw.contains(PageFlags::DIRTY));
        assert!(!rw.contains(PageFlags::EXECUTE));

        let rx = PageFlags::kernel_rx();
        assert!(rx.contains(PageFlags::VALID));
        assert!(rx.contains(PageFlags::READ));
        assert!(rx.contains(PageFlags::EXECUTE));
        assert!(!rx.contains(PageFlags::WRITE));

        let rwx = PageFlags::kernel_rwx();
        assert!(rwx.contains(PageFlags::READ));
        assert!(rwx.contains(PageFlags::WRITE));
        assert!(rwx.contains(PageFlags::EXECUTE));
    }

    #[test]
    fn pte_is_valid_and_leaf() {
        let pa = PhysAddr::new(0x0000_1000);
        let leaf = PageTableEntry::new(pa, PageFlags::VALID | PageFlags::READ);
        assert!(leaf.is_valid());
        assert!(leaf.is_leaf());

        let intermediate = PageTableEntry::new(pa, PageFlags::VALID);
        assert!(intermediate.is_valid());
        assert!(!intermediate.is_leaf());
    }
}
