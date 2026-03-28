//! 测试用页表实现——使用堆分配模拟物理帧，可在宿主机上测试 walk/map/unmap 逻辑。

use super::{PageFlags, PageTableEntry};
use crate::address::{PhysAddr, VirtAddr};
use crate::error::MemoryError;
use config::PAGE_SIZE;

const ENTRIES_PER_PAGE: usize = PAGE_SIZE / 8;
const PT_LEVELS: usize = 3;

#[inline]
fn vpn_index(va: VirtAddr, level: usize) -> usize {
    (va.as_usize() >> (12 + level * 9)) & 0x1FF
}

/// 堆分配的帧，模拟裸机环境的 `FrameTracker`。
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

/// 测试用页表，API 与裸机 `PageTable` 相同，底层用堆分配代替帧分配器。
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

    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, MemoryError> {
        let pte = self.find_pte_mut(va).ok_or(MemoryError::PageNotMapped)?;
        if !pte.is_valid() {
            return Err(MemoryError::PageNotMapped);
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
