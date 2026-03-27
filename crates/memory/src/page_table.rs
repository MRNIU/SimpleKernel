//! 多级页表抽象。
//!
//! `PageFlags` 和 `PageTableEntry` 的结构体定义在此处（架构无关）。
//! `PageTableEntry` 的编码实现（Sv39 / ARMv8）在各架构的 `pte.rs` 中。
//! `PageTable`（页表 walk/map/unmap 逻辑）也在此处，通过
//! `PageTableEntry::new_intermediate()` 屏蔽中间节点的架构差异。

use bitflags::bitflags;

use crate::address::PhysAddr;

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

// ─── PTE 编码（原在 arch/*/pte.rs，迁入以打破 memory↔arch 循环） ───

#[cfg(all(not(test), target_arch = "riscv64"))]
mod pte_encoding {
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
}

#[cfg(all(not(test), target_arch = "aarch64"))]
mod pte_encoding {
    use super::{PageFlags, PageTableEntry};
    use crate::address::PhysAddr;

    const VALID_BIT: u64 = 1 << 0;
    const TABLE_BIT: u64 = 1 << 1;
    const AF_BIT: u64 = 1 << 10;
    const SH_INNER: u64 = 0b11 << 8;
    const MAIR_IDX0: u64 = 0b000 << 2;
    const AP_RO: u64 = 0b10 << 6;
    const AP_RW: u64 = 0b00 << 6;
    const PXN_BIT: u64 = 1 << 53;
    const UXN_BIT: u64 = 1 << 54;
    const OUTPUT_ADDR_MASK: u64 = 0x0000_FFFF_FFFF_F000;

    impl PageTableEntry {
        pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
            let mut bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK)
                | VALID_BIT
                | TABLE_BIT
                | AF_BIT
                | SH_INNER
                | MAIR_IDX0;
            if flags.contains(PageFlags::WRITE) {
                bits |= AP_RW;
            } else {
                bits |= AP_RO;
            }
            if flags.contains(PageFlags::USER) {
                bits |= 0b01 << 6;
            }
            if !flags.contains(PageFlags::EXECUTE) {
                bits |= PXN_BIT | UXN_BIT;
            }
            if !flags.contains(PageFlags::GLOBAL) {
                bits |= 1 << 11;
            }
            Self(bits)
        }
        #[inline]
        pub fn new_table(paddr: PhysAddr) -> Self {
            let bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK) | VALID_BIT | TABLE_BIT;
            Self(bits)
        }
        #[inline]
        pub fn paddr(self) -> PhysAddr {
            PhysAddr::new((self.0 & OUTPUT_ADDR_MASK) as usize)
        }
        pub fn flags(self) -> PageFlags {
            let mut f = PageFlags::empty();
            if self.is_valid() {
                f |= PageFlags::VALID;
            }
            let ap = (self.0 >> 6) & 0b11;
            f |= PageFlags::READ;
            if ap & 0b10 == 0 {
                f |= PageFlags::WRITE;
                f |= PageFlags::DIRTY;
            }
            if ap & 0b01 != 0 {
                f |= PageFlags::USER;
            }
            if self.0 & PXN_BIT == 0 {
                f |= PageFlags::EXECUTE;
            }
            if self.0 & AF_BIT != 0 {
                f |= PageFlags::ACCESSED;
            }
            if self.0 & (1 << 11) == 0 {
                f |= PageFlags::GLOBAL;
            }
            f
        }
        #[inline]
        pub fn is_valid(self) -> bool {
            self.0 & VALID_BIT != 0
        }
        #[inline]
        pub fn is_leaf(self) -> bool {
            self.is_valid() && (self.0 & AF_BIT != 0)
        }
        #[inline]
        pub fn empty() -> Self {
            Self(0)
        }
        #[inline]
        pub fn new_intermediate(paddr: PhysAddr) -> Self {
            Self::new_table(paddr)
        }
    }
}

#[cfg(target_os = "none")]
mod inner {
    use super::{PageFlags, PageTableEntry};
    use crate::address::{PhysAddr, VirtAddr};
    use crate::frame::FrameTracker;
    use config::PAGE_SIZE;
    use error::{ErrorCode, KResult};

    /// 每页 PTE 数量（4KB / 8 = 512）
    pub const ENTRIES_PER_PAGE: usize = PAGE_SIZE / 8;

    /// 页表层级数——从 config crate 获取
    pub const PT_LEVELS: usize = config::PT_LEVELS;

    /// 从虚拟地址中提取第 `level` 级的 9 位 VPN 索引。
    #[inline]
    pub fn vpn_index(va: VirtAddr, level: usize) -> usize {
        (va.as_usize() >> (12 + level * 9)) & 0x1FF
    }

    /// 将物理地址解释为页表项数组。
    ///
    /// # Safety
    /// - `paddr` 必须指向有效、页对齐、由 `FrameTracker` 分配的帧，
    ///   且当前不存在其他可变引用。
    /// - 当前使用 identity mapping（VA == PA），物理地址可直接作为虚拟地址解引用。
    ///   若未来切换为非 identity mapping，此处需通过 `phys_to_virt()` 转换。
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

        /// 可变遍历，不分配。用于 unmap 等无需创建中间节点的场景。
        pub fn find_pte_mut(&mut self, va: VirtAddr) -> Option<&'static mut PageTableEntry> {
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
            Some(&mut table[idx])
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
            let pte = self.find_pte_mut(va).ok_or(ErrorCode::VmPageNotMapped)?;
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

#[cfg(target_os = "none")]
pub use inner::PageTable;

/// 测试用页表实现——使用堆分配模拟物理帧，可在宿主机上测试 walk/map/unmap 逻辑。
#[cfg(test)]
mod test_page_table {
    use super::{PageFlags, PageTableEntry};
    use crate::address::{PhysAddr, VirtAddr};
    use config::PAGE_SIZE;
    use error::{ErrorCode, KResult};

    const ENTRIES_PER_PAGE: usize = PAGE_SIZE / 8;
    /// Sv39 三级页表
    const PT_LEVELS: usize = 3;

    #[inline]
    fn vpn_index(va: VirtAddr, level: usize) -> usize {
        (va.as_usize() >> (12 + level * 9)) & 0x1FF
    }

    /// 堆分配的帧——模拟 FrameTracker
    struct TestFrame {
        data: Box<[u8; PAGE_SIZE]>,
    }

    impl TestFrame {
        fn alloc() -> Self {
            Self {
                data: Box::new([0u8; PAGE_SIZE]),
            }
        }

        fn paddr(&self) -> PhysAddr {
            PhysAddr::new(self.data.as_ptr() as usize)
        }
    }

    /// # Safety
    /// `paddr` 必须指向 TestFrame 分配的堆内存。
    unsafe fn pte_array(paddr: PhysAddr) -> &'static mut [PageTableEntry; ENTRIES_PER_PAGE] {
        unsafe { &mut *(paddr.as_usize() as *mut [PageTableEntry; ENTRIES_PER_PAGE]) }
    }

    /// 测试用页表
    pub struct TestPageTable {
        root: TestFrame,
        frames: Vec<TestFrame>,
    }

    impl TestPageTable {
        pub fn new() -> Self {
            Self {
                root: TestFrame::alloc(),
                frames: Vec::new(),
            }
        }

        pub fn root_paddr(&self) -> PhysAddr {
            self.root.paddr()
        }

        pub fn find_or_create_pte(&mut self, va: VirtAddr) -> KResult<&'static mut PageTableEntry> {
            let mut paddr = self.root.paddr();

            for level in (1..PT_LEVELS).rev() {
                let table = unsafe { pte_array(paddr) };
                let idx = vpn_index(va, level);
                let pte = &mut table[idx];

                if !pte.is_valid() {
                    let frame = TestFrame::alloc();
                    let frame_paddr = frame.paddr();
                    *pte = PageTableEntry::new_intermediate(frame_paddr);
                    self.frames.push(frame);
                }

                paddr = pte.paddr();
            }

            let table = unsafe { pte_array(paddr) };
            let idx = vpn_index(va, 0);
            Ok(&mut table[idx])
        }

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

        pub fn find_pte_mut(&mut self, va: VirtAddr) -> Option<&'static mut PageTableEntry> {
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
            Some(&mut table[idx])
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
            let pte = self.find_pte_mut(va).ok_or(ErrorCode::VmPageNotMapped)?;
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

#[cfg(test)]
mod tests {
    use super::test_page_table::TestPageTable;
    use super::*;
    use crate::address::VirtAddr;
    use error::ErrorCode;

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

    // ─── PageTable walk/map/unmap 测试 ──────────────────────────────────────

    #[test]
    fn map_and_get_mapping() {
        let mut pt = TestPageTable::new();
        let va = VirtAddr::new(0x1000); // 第一个用户页
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PageFlags::kernel_rw();

        pt.map_page(va, pa, flags).expect("map_page 应成功");

        let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能找到映射");
        assert_eq!(mapped_pa, pa);
        assert!(mapped_flags.contains(PageFlags::VALID));
        assert!(mapped_flags.contains(PageFlags::READ));
        assert!(mapped_flags.contains(PageFlags::WRITE));
    }

    #[test]
    fn map_different_pages() {
        let mut pt = TestPageTable::new();

        // 映射两个不同的虚拟页到不同的物理页
        let va1 = VirtAddr::new(0x0000_1000);
        let va2 = VirtAddr::new(0x0000_2000);
        let pa1 = PhysAddr::new(0x8020_0000);
        let pa2 = PhysAddr::new(0x8020_1000);

        pt.map_page(va1, pa1, PageFlags::kernel_rw())
            .expect("map va1");
        pt.map_page(va2, pa2, PageFlags::kernel_rx())
            .expect("map va2");

        let (got_pa1, _) = pt.get_mapping(va1).expect("va1 应已映射");
        let (got_pa2, _) = pt.get_mapping(va2).expect("va2 应已映射");
        assert_eq!(got_pa1, pa1);
        assert_eq!(got_pa2, pa2);
    }

    #[test]
    fn double_map_fails() {
        let mut pt = TestPageTable::new();
        let va = VirtAddr::new(0x1000);
        let pa = PhysAddr::new(0x8020_0000);

        pt.map_page(va, pa, PageFlags::kernel_rw())
            .expect("首次 map 应成功");
        let err = pt
            .map_page(va, pa, PageFlags::kernel_rw())
            .expect_err("重复 map 应失败");
        assert_eq!(err, ErrorCode::VmMapFailed);
    }

    #[test]
    fn unmap_page_returns_old_pa() {
        let mut pt = TestPageTable::new();
        let va = VirtAddr::new(0x1000);
        let pa = PhysAddr::new(0x8020_0000);

        pt.map_page(va, pa, PageFlags::kernel_rw())
            .expect("map 应成功");
        let old_pa = pt.unmap_page(va).expect("unmap 应成功");
        assert_eq!(old_pa, pa);

        // unmap 后再查询应为 None（PTE 已清零，is_leaf 为 false）
        assert!(pt.get_mapping(va).is_none());
    }

    #[test]
    fn unmap_unmapped_page_fails() {
        let mut pt = TestPageTable::new();
        let va = VirtAddr::new(0x1000);

        let err = pt.unmap_page(va).expect_err("unmap 未映射页应失败");
        assert_eq!(err, ErrorCode::VmPageNotMapped);
    }

    #[test]
    fn map_pages_in_different_vpn_ranges() {
        let mut pt = TestPageTable::new();

        // 跨不同 VPN[2] 范围的地址，会触发不同的二级页表分配
        let va_low = VirtAddr::new(0x0000_1000);
        let va_high = VirtAddr::new(0x4000_0000); // VPN[2] = 1
        let pa1 = PhysAddr::new(0x8020_0000);
        let pa2 = PhysAddr::new(0x8020_1000);

        pt.map_page(va_low, pa1, PageFlags::kernel_rw())
            .expect("map low");
        pt.map_page(va_high, pa2, PageFlags::kernel_rw())
            .expect("map high");

        let (got1, _) = pt.get_mapping(va_low).expect("low 应已映射");
        let (got2, _) = pt.get_mapping(va_high).expect("high 应已映射");
        assert_eq!(got1, pa1);
        assert_eq!(got2, pa2);
    }
}
