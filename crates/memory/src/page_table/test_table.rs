//! 测试用页表实现——使用堆分配模拟物理帧，可在宿主机上测试 walk/map/unmap 逻辑。

use super::{PageFlags, PageTableEntry, vpn_index};
use crate::address::{PhysAddr, VirtAddr};
use crate::error::MemoryError;
use config::PAGE_SIZE;

const PT_LEVELS: usize = 3;

/// 堆分配的帧，模拟裸机环境的 `FrameTracker`。
///
/// 使用页对齐分配——PTE 编码会截断低 12 位，
/// 非对齐地址经 PTE 往返后会丢失偏移量。
struct TestFrame {
    ptr: *mut u8,
    layout: core::alloc::Layout,
}

impl TestFrame {
    fn alloc() -> Self {
        let layout = core::alloc::Layout::from_size_align(PAGE_SIZE, PAGE_SIZE)
            .expect("TestFrame: invalid layout");
        // SAFETY: layout 非零大小
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        assert!(!ptr.is_null(), "TestFrame: allocation failed");
        Self { ptr, layout }
    }

    fn paddr(&self) -> PhysAddr {
        PhysAddr::new(self.ptr as usize)
    }
}

impl Drop for TestFrame {
    fn drop(&mut self) {
        // SAFETY: ptr 由同 layout 的 alloc_zeroed 分配
        unsafe { std::alloc::dealloc(self.ptr, self.layout) };
    }
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

    fn find_or_create_pte(&mut self, va: VirtAddr) -> Result<*mut PageTableEntry, MemoryError> {
        let mut base = self.root.paddr().as_usize() as *mut PageTableEntry;

        for level in (1..PT_LEVELS).rev() {
            let idx = vpn_index(va, level);
            // SAFETY: base 指向 TestFrame 拥有的堆内存，idx < 512
            let pte = unsafe { base.add(idx).read() };

            if !pte.is_valid() {
                let frame = TestFrame::alloc();
                let frame_paddr = frame.paddr();
                // SAFETY: 同上
                unsafe {
                    base.add(idx)
                        .write(PageTableEntry::new_intermediate(frame_paddr));
                }
                self.frames.push(frame);
                base = frame_paddr.as_usize() as *mut PageTableEntry;
            } else {
                base = pte.paddr().as_usize() as *mut PageTableEntry;
            }
        }

        let idx = vpn_index(va, 0);
        // SAFETY: base 指向有效帧，idx < 512
        Ok(unsafe { base.add(idx) })
    }

    fn find_pte(&self, va: VirtAddr) -> Option<PageTableEntry> {
        let mut base = self.root.paddr().as_usize() as *const PageTableEntry;

        for level in (1..PT_LEVELS).rev() {
            let idx = vpn_index(va, level);
            // SAFETY: base 指向 TestFrame 拥有的堆内存
            let pte = unsafe { base.add(idx).read() };
            if !pte.is_valid() {
                return None;
            }
            base = pte.paddr().as_usize() as *const PageTableEntry;
        }

        let idx = vpn_index(va, 0);
        // SAFETY: 同上
        Some(unsafe { base.add(idx).read() })
    }

    fn find_pte_mut(&mut self, va: VirtAddr) -> Option<*mut PageTableEntry> {
        let mut base = self.root.paddr().as_usize() as *mut PageTableEntry;

        for level in (1..PT_LEVELS).rev() {
            let idx = vpn_index(va, level);
            // SAFETY: base 指向 TestFrame 拥有的堆内存
            let pte = unsafe { base.add(idx).read() };
            if !pte.is_valid() {
                return None;
            }
            base = pte.paddr().as_usize() as *mut PageTableEntry;
        }

        let idx = vpn_index(va, 0);
        // SAFETY: 同上
        Some(unsafe { base.add(idx) })
    }

    pub fn map_page(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), MemoryError> {
        let pte_ptr = self.find_or_create_pte(va)?;
        // SAFETY: pte_ptr 指向 TestFrame 持有的堆内存
        let current = unsafe { pte_ptr.read() };
        if current.is_valid() {
            return Err(MemoryError::MapFailed);
        }
        unsafe { pte_ptr.write(PageTableEntry::new(pa, flags)) };
        Ok(())
    }

    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, MemoryError> {
        let pte_ptr = self.find_pte_mut(va).ok_or(MemoryError::PageNotMapped)?;
        // SAFETY: pte_ptr 指向 TestFrame 持有的堆内存
        let pte = unsafe { pte_ptr.read() };
        if !pte.is_valid() {
            return Err(MemoryError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        unsafe { pte_ptr.write(PageTableEntry::empty()) };
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
