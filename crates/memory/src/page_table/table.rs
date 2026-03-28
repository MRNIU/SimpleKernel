//! 裸机页表——walk / map / unmap 逻辑。

use super::{Level0, PageFlags, PageTableEntry, Table, vpn_index};
use crate::address::{PhysAddr, VirtAddr};
use crate::error::MemoryError;
use crate::frame::AllocatedFrame;

const PT_LEVELS: usize = config::PT_LEVELS;

/// 从物理地址构造 `Table<Level0>` 用于 walker 内部。
///
/// walker 在运行时循环中无法使用编译期类型参数区分层级，
/// 统一使用 `Level0` 是因为当前所有层级的 entries 数量相同（均从 PAGE_SIZE 推导）。
/// 索引计算通过 [`vpn_index`] + [`LEVEL_INFO`] 查表实现，不依赖 `L::ENTRIES`。
///
/// # Safety
/// `paddr` 必须指向有效、页对齐的帧，当前使用 identity mapping（VA == PA）。
#[inline]
unsafe fn table_at(paddr: PhysAddr) -> Table<Level0> {
    // SAFETY: 调用方保证 paddr 有效
    unsafe { Table::<Level0>::from_paddr(paddr) }
}

/// 多级页表。
///
/// 拥有根帧及所有遍历过程中分配的中间帧。
/// drop 时自动归还所有帧。
pub struct PageTable {
    root: AllocatedFrame,
    frames: alloc::vec::Vec<AllocatedFrame>,
}

impl PageTable {
    /// 创建新页表，分配根帧。
    pub fn new() -> Result<Self, MemoryError> {
        let root = AllocatedFrame::alloc()?;
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

    /// 遍历到 `va` 对应的叶 PTE，必要时分配中间节点。返回 PTE 裸指针。
    fn find_or_create_pte(&mut self, va: VirtAddr) -> Result<*mut PageTableEntry, MemoryError> {
        let mut paddr = self.root.paddr();

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self.root 或 self.frames 持有的有效帧
            let mut table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = AllocatedFrame::alloc()?;
                let frame_paddr = frame.paddr();
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                self.frames.push(frame);
                paddr = frame_paddr;
            } else {
                paddr = pte.paddr();
            }
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, 0);
        Ok(table.entry_ptr(idx))
    }

    /// 只读遍历，不分配。返回 PTE 值的拷贝。
    fn find_pte(&self, va: VirtAddr) -> Option<PageTableEntry> {
        let mut paddr = self.root.paddr();

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return None;
            }
            paddr = pte.paddr();
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, 0);
        Some(table.read(idx))
    }

    /// 可变遍历，不分配。返回叶 PTE 裸指针。
    fn find_pte_mut(&mut self, va: VirtAddr) -> Option<*mut PageTableEntry> {
        let mut paddr = self.root.paddr();

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return None;
            }
            paddr = pte.paddr();
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, 0);
        Some(table.entry_ptr(idx))
    }

    /// 映射单个虚拟页到物理帧。若该 VA 已被映射则返回错误。
    pub fn map_page(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), MemoryError> {
        let pte_ptr = self.find_or_create_pte(va)?;
        // SAFETY: pte_ptr 指向 self 持有的帧内存
        let current = unsafe { pte_ptr.read() };
        if current.is_valid() {
            return Err(MemoryError::MapFailed);
        }
        // SAFETY: 同上
        unsafe { pte_ptr.write(PageTableEntry::new(pa, flags)) };
        Ok(())
    }

    /// 取消映射单个虚拟页，返回其原始物理地址。
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, MemoryError> {
        let pte_ptr = self.find_pte_mut(va).ok_or(MemoryError::PageNotMapped)?;
        // SAFETY: pte_ptr 指向 self 持有的帧内存
        let pte = unsafe { pte_ptr.read() };
        if !pte.is_valid() {
            return Err(MemoryError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        // SAFETY: 同上
        unsafe { pte_ptr.write(PageTableEntry::empty()) };
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
