//! 裸机页表——walk / map / unmap 逻辑。

use super::{PageFlags, PageTableEntry};
use crate::address::{PhysAddr, VirtAddr};
use crate::error::MemoryError;
use crate::frame::FrameTracker;
use config::PAGE_SIZE;

const ENTRIES_PER_PAGE: usize = PAGE_SIZE / 8;
const PT_LEVELS: usize = config::PT_LEVELS;

/// 从虚拟地址中提取第 `level` 级的 9 位 VPN 索引。
#[inline]
fn vpn_index(va: VirtAddr, level: usize) -> usize {
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
    /// 创建新页表，分配根帧。
    pub fn new() -> Result<Self, MemoryError> {
        let root = FrameTracker::alloc()?;
        Ok(Self {
            root,
            frames: alloc::vec::Vec::new(),
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root.paddr()
    }

    /// 遍历到 `va` 对应的叶 PTE，必要时分配中间节点。
    pub fn find_or_create_pte(
        &mut self,
        va: VirtAddr,
    ) -> Result<&'static mut PageTableEntry, MemoryError> {
        let mut paddr = self.root.paddr();

        for level in (1..PT_LEVELS).rev() {
            let table = unsafe { pte_array(paddr) };
            let idx = vpn_index(va, level);
            let pte = &mut table[idx];

            if !pte.is_valid() {
                let frame = FrameTracker::alloc()?;
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

    /// 映射单个虚拟页到物理帧。若该 VA 已被映射则返回错误。
    pub fn map_page(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), MemoryError> {
        let pte = self.find_or_create_pte(va)?;
        if pte.is_valid() {
            return Err(MemoryError::MapFailed);
        }
        *pte = PageTableEntry::new(pa, flags);
        Ok(())
    }

    /// 取消映射单个虚拟页，返回其原始物理地址。
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, MemoryError> {
        let pte = self.find_pte_mut(va).ok_or(MemoryError::PageNotMapped)?;
        if !pte.is_valid() {
            return Err(MemoryError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        *pte = PageTableEntry::empty();
        Ok(old_pa)
    }

    /// 查询虚拟地址的映射信息，返回物理地址和标志。
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PageFlags)> {
        let pte = self.find_pte(va)?;
        if pte.is_valid() && pte.is_leaf() {
            Some((pte.paddr(), pte.flags()))
        } else {
            None
        }
    }
}
