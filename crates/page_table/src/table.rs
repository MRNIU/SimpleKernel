//! 多级页表——walk / map / unmap 逻辑。
//!
//! `PageTable<F>` 对帧分配的依赖通过 [`NodeFrameOps`] trait 泛型化——
//! 裸机和宿主机测试共享同一份 walk 实现。
//!
//! `unmap_at_level` 在清除叶 PTE 后回溯检查中间节点是否全空，
//! 若是则清除上级 PTE 并回收该帧——参考 Linux `free_pgtables()`。

extern crate alloc;

use alloc::collections::BTreeMap;

use crate::error::PageTableError;
use crate::{
    Level0, NodeFrameOps, PageLevel, PageTableEntry, PteFlags, PteFlagsOps, PteOps, Table,
    vpn_index,
};
use address::{PhysAddr, VirtAddr};

const PT_LEVELS: usize = config::PT_LEVELS;

/// 从物理地址构造 `Table<Level0>` 用于 walker 内部。
///
/// # Safety
/// `paddr` 必须指向有效、页对齐的帧。
#[inline]
unsafe fn table_at(paddr: PhysAddr) -> Table<Level0> {
    // SAFETY: 调用方保证 paddr 有效
    unsafe { Table::<Level0>::from_paddr(paddr) }
}

/// 多级页表。
///
/// 拥有根帧及所有遍历过程中分配的中间帧。
/// drop 时自动归还所有帧。
///
/// 类型参数 `F` 是帧类型——裸机使用物理帧分配器的帧，测试使用堆分配帧。
/// 消费方通过类型别名隐藏泛型参数：
/// ```ignore
/// type PageTable = page_table::PageTable<AllocatedFrames>;
/// ```
pub struct PageTable<F: NodeFrameOps> {
    root_paddr: PhysAddr,
    /// 持有根帧所有权，阻止帧被释放——字段本身不直接访问。
    #[expect(dead_code, reason = "仅用于持有所有权，通过 root_paddr 访问")]
    root: F,
    /// 中间页表节点——以物理地址为键，O(log n) 查找/删除。
    frames: BTreeMap<PhysAddr, F>,
}

impl<F: NodeFrameOps> PageTable<F> {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, PageTableError> {
        let root = F::alloc()?;
        let root_paddr = root.paddr();
        Ok(Self {
            root_paddr,
            root,
            frames: BTreeMap::new(),
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    #[cfg(target_os = "none")]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root_paddr
    }

    /// 映射用 walker——遍历到 `target_level` 并按需分配中间节点。
    fn walk_create(
        &mut self,
        va: VirtAddr,
        target_level: usize,
    ) -> Result<*mut PageTableEntry, PageTableError> {
        let mut paddr = self.root_paddr;

        for level in (target_level + 1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self.root 或 self.frames 持有的有效帧
            let mut table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = F::alloc()?;
                let frame_paddr = frame.paddr();
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                self.frames.insert(frame_paddr, frame);
                paddr = frame_paddr;
            } else if pte.is_leaf(level) {
                return Err(PageTableError::MapFailed);
            } else {
                paddr = pte.paddr();
            }
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, target_level);
        Ok(table.entry_ptr(idx))
    }

    /// 映射单个虚拟页到物理帧（Level 0，4KB）。
    pub fn map_page(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PageTableError> {
        self.map_at_level(va, pa, flags, 0)
    }

    /// 在指定层级映射虚拟地址到物理地址。
    ///
    /// - `level = 0`：4KB 页
    /// - `level = 1`：2MB 大页
    /// - `level = 2`：1GB 大页
    ///
    /// # Errors
    ///
    /// 该 VA 已被映射时返回 `MapFailed`。
    pub fn map_at_level(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
        level: usize,
    ) -> Result<(), PageTableError> {
        let pte_ptr = self.walk_create(va, level)?;
        // SAFETY: walk_create 返回的指针指向 self 持有的帧内存
        let current = unsafe { pte_ptr.read() };
        if current.is_valid() {
            return Err(PageTableError::MapFailed);
        }
        let leaf_flags = flags.for_leaf_at_level(level);
        // SAFETY: pte_ptr 指向 self 持有的帧内存，上方已检查无冲突映射
        unsafe { pte_ptr.write(PageTableEntry::new(pa, leaf_flags)) };
        Ok(())
    }

    /// 取消映射单个虚拟页（4KB），返回其原始物理地址。
    ///
    /// # Errors
    ///
    /// 目标 VA 未映射时返回 `PageNotMapped`。
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, PageTableError> {
        self.unmap_at_level(va, 0)
    }

    /// 在指定层级取消映射，返回原始物理地址。
    ///
    /// unmap 后自动检查中间页表节点是否全空并回收。
    ///
    /// **注意**：当前不支持大页分裂（transparent huge page splitting）——
    /// 不能 unmap 大页的一部分。如需部分 unmap，须先手动将大页分裂为小页。
    /// 此限制在引入 THP 支持前保持不变。
    ///
    /// # Errors
    ///
    /// 目标 VA 在指定层级未映射时返回 `PageNotMapped`。
    pub fn unmap_at_level(
        &mut self,
        va: VirtAddr,
        level: usize,
    ) -> Result<PhysAddr, PageTableError> {
        let mut path: [(PhysAddr, usize, PhysAddr); 4] =
            [(PhysAddr::new(0), 0, PhysAddr::new(0)); 4];
        let mut path_len = 0;
        let mut paddr = self.root_paddr;

        for lv in (level + 1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, lv);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return Err(PageTableError::PageNotMapped);
            }
            if pte.is_leaf(lv) {
                return Err(PageTableError::PageNotMapped);
            }
            let child_paddr = pte.paddr();
            path[path_len] = (paddr, idx, child_paddr);
            path_len += 1;
            paddr = child_paddr;
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, level);
        let pte = table.read(idx);
        if !pte.is_valid() || !pte.is_leaf(level) {
            return Err(PageTableError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        table.write(idx, PageTableEntry::empty());

        let entries_per_table = Level0::ENTRIES;
        let mut child_paddr = paddr;
        for &(parent_paddr, parent_idx, _) in path[..path_len].iter().rev() {
            // SAFETY: child_paddr 指向由 self 持有的有效帧
            let child_table = unsafe { table_at(child_paddr) };
            let all_empty = (0..entries_per_table).all(|j| !child_table.read(j).is_valid());
            if !all_empty {
                break;
            }
            // SAFETY: parent_paddr 指向由 self 持有的有效帧
            let mut parent_table = unsafe { table_at(parent_paddr) };
            parent_table.write(parent_idx, PageTableEntry::empty());
            self.frames.remove(&child_paddr);
            child_paddr = parent_paddr;
        }

        Ok(old_pa)
    }

    /// 只读遍历——从根向下查找叶 PTE，返回 PTE 及其所在层级。
    fn walk_readonly(&self, va: VirtAddr) -> Option<(PageTableEntry, usize)> {
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return None;
            }
            if pte.is_leaf(level) {
                return Some((pte, level));
            }
            paddr = pte.paddr();
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, 0);
        let pte = table.read(idx);
        if pte.is_valid() && pte.is_leaf(0) {
            Some((pte, 0))
        } else {
            None
        }
    }

    /// 查询虚拟地址的映射信息，返回物理地址和标志。
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
        let (pte, level) = self.walk_readonly(va)?;
        let page_size = crate::page_size_at_level(level);
        let offset = va.as_usize() & (page_size - 1);
        Some((pte.paddr() + offset, pte.flags()))
    }

    /// 将 `[start, end)` 物理地址区间 identity-map（VA == PA）。
    ///
    /// 自动使用最大可用页大小（1GB / 2MB / 4KB）。
    ///
    /// # Errors
    ///
    /// 映射冲突或 `start >= end` 时返回 `MapFailed`。
    /// 失败时已建立的部分映射**不会回滚**——调用方应 panic 或处理不一致状态。
    pub fn identity_map_range(
        &mut self,
        start: PhysAddr,
        end: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PageTableError> {
        let mut addr = start.align_down();
        let end_aligned = end.align_up();

        if addr.as_usize() >= end_aligned.as_usize() {
            return Err(PageTableError::MapFailed);
        }

        while addr.as_usize() < end_aligned.as_usize() {
            let remaining = end_aligned.as_usize() - addr.as_usize();
            let va = VirtAddr::new(addr.as_usize());

            let mut mapped = false;
            for level in (1..PT_LEVELS).rev() {
                let page_size = crate::page_size_at_level(level);
                if addr.as_usize().is_multiple_of(page_size) && remaining >= page_size {
                    self.map_at_level(va, addr, flags, level)?;
                    addr += page_size;
                    mapped = true;
                    break;
                }
            }
            if !mapped {
                self.map_page(va, addr, flags)?;
                addr += config::PAGE_SIZE;
            }
        }
        Ok(())
    }
}
