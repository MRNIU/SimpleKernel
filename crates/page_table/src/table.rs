//! 多级页表——walk / map / unmap 逻辑。
//!
//! `PageTable<F>` 对帧分配的依赖通过 [`NodeFrameOps`] trait 泛型化——
//! 裸机和宿主机测试共享同一份 walk 实现。
//!
//! `unmap_at_level` 在清除叶 PTE 后通过引用计数判断中间节点是否全空，
//! 若是则清除上级 PTE 并回收该帧——参考 Linux `free_pgtables()` + `struct page::_mapcount`。
//!
//! **大页分裂**：当前不支持 transparent huge page splitting——
//! 不能 unmap 大页的一部分，也不能在大页覆盖范围内映射小页。
//! 如需部分 unmap，须先手动将大页分裂为小页再操作。
//! 此限制在引入 THP 支持前保持不变。

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::error::PageTableError;
use crate::{NodeFrameOps, PageTableEntry, PteFlags, PteFlagsOps, PteOps, Table, vpn_index};
use address::{PhysAddr, VirtAddr};

const PT_LEVELS: usize = config::PT_LEVELS;

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
    /// 每个页表帧（含根帧）中有效 PTE 的引用计数。
    /// map 时 +1，unmap 时 -1，count == 0 且非根帧时可回收。
    /// 避免 unmap 回溯时 O(entries_per_table) 全扫描。
    ref_counts: BTreeMap<PhysAddr, u16>,
}

impl<F: NodeFrameOps> PageTable<F> {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, PageTableError> {
        let root = F::alloc()?;
        let root_paddr = root.paddr();
        let mut ref_counts = BTreeMap::new();
        ref_counts.insert(root_paddr, 0);
        Ok(Self {
            root_paddr,
            root,
            frames: BTreeMap::new(),
            ref_counts,
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root_paddr
    }

    /// 递增指定帧的引用计数。
    #[inline]
    fn inc_ref(&mut self, paddr: PhysAddr) {
        *self
            .ref_counts
            .get_mut(&paddr)
            .expect("ref_counts: 帧未注册") += 1;
    }

    /// 递减指定帧的引用计数，返回递减后的值。
    #[inline]
    fn dec_ref(&mut self, paddr: PhysAddr) -> u16 {
        let count = self
            .ref_counts
            .get_mut(&paddr)
            .expect("ref_counts: 帧未注册");
        *count -= 1;
        *count
    }

    /// 映射用 walker——遍历到 `target_level` 并按需分配中间节点。
    ///
    /// 返回目标 PTE 所在帧的物理地址及该 PTE 在帧中的索引。
    fn walk_create(
        &mut self,
        va: VirtAddr,
        target_level: usize,
    ) -> Result<(PhysAddr, usize), PageTableError> {
        let mut paddr = self.root_paddr;

        for level in (target_level + 1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self.root 或 self.frames 持有的有效帧
            let mut table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = F::alloc()?;
                let frame_paddr = frame.paddr();
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                self.frames.insert(frame_paddr, frame);
                self.ref_counts.insert(frame_paddr, 0);
                self.inc_ref(paddr);
                paddr = frame_paddr;
            } else if pte.is_leaf(level) {
                return Err(PageTableError::HugePageConflict);
            } else {
                paddr = pte.paddr();
            }
        }

        let idx = vpn_index(va, target_level);
        Ok((paddr, idx))
    }

    /// 映射单个虚拟页到物理帧（Level 0，4KB）。
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Errors
    ///
    /// - 该 VA 已被映射时返回 `AlreadyMapped`。
    /// - walk 路径上遇到大页时返回 `HugePageConflict`。
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
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Errors
    ///
    /// - 该 VA 已被映射时返回 `AlreadyMapped`。
    /// - walk 路径上遇到大页时返回 `HugePageConflict`。
    pub fn map_at_level(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
        level: usize,
    ) -> Result<(), PageTableError> {
        // 在执行任何操作前检查 VA/PA 对齐
        let page_size = crate::page_size_at_level(level);
        debug_assert!(
            va.as_usize().is_multiple_of(page_size),
            "map_at_level: VA {:#x} 未按 level {} 页大小 ({:#x}) 对齐",
            va.as_usize(),
            level,
            page_size
        );
        debug_assert!(
            pa.as_usize().is_multiple_of(page_size),
            "map_at_level: PA {:#x} 未按 level {} 页大小 ({:#x}) 对齐",
            pa.as_usize(),
            level,
            page_size
        );

        let (frame_paddr, idx) = self.walk_create(va, level)?;
        // SAFETY: frame_paddr 指向由 self 持有的有效帧
        let mut table = unsafe { Table::from_paddr(frame_paddr) };
        let current = table.read(idx);
        if current.is_valid() {
            return Err(PageTableError::AlreadyMapped);
        }
        let leaf_flags = flags.for_leaf_at_level(level);
        table.write(idx, PageTableEntry::new(pa, leaf_flags));
        self.inc_ref(frame_paddr);
        Ok(())
    }

    /// 取消映射单个虚拟页（4KB），返回其原始物理地址。
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Errors
    ///
    /// 目标 VA 未映射时返回 `PageNotMapped`。
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, PageTableError> {
        self.unmap_at_level(va, 0)
    }

    /// 在指定层级取消映射，返回原始物理地址。
    ///
    /// unmap 后通过引用计数判断中间页表节点是否全空并回收，
    /// 避免遍历整个页表帧的 O(entries_per_table) 开销。
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Errors
    ///
    /// 目标 VA 在指定层级未映射时返回 `PageNotMapped`。
    pub fn unmap_at_level(
        &mut self,
        va: VirtAddr,
        level: usize,
    ) -> Result<PhysAddr, PageTableError> {
        let mut path: [(PhysAddr, usize, PhysAddr); PT_LEVELS] =
            [(PhysAddr::new(0), 0, PhysAddr::new(0)); PT_LEVELS];
        let mut path_len = 0;
        let mut paddr = self.root_paddr;

        for lv in (level + 1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
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
        let mut table = unsafe { Table::from_paddr(paddr) };
        let idx = vpn_index(va, level);
        let pte = table.read(idx);
        if !pte.is_valid() || !pte.is_leaf(level) {
            return Err(PageTableError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        table.write(idx, PageTableEntry::empty());
        self.dec_ref(paddr);

        let mut child_paddr = paddr;
        for &(parent_paddr, parent_idx, _) in path[..path_len].iter().rev() {
            let count = *self
                .ref_counts
                .get(&child_paddr)
                .expect("ref_counts: 帧未注册");
            if count > 0 {
                break;
            }
            // SAFETY: parent_paddr 指向由 self 持有的有效帧
            let mut parent_table = unsafe { Table::from_paddr(parent_paddr) };
            parent_table.write(parent_idx, PageTableEntry::empty());
            self.frames.remove(&child_paddr);
            self.ref_counts.remove(&child_paddr);
            self.dec_ref(parent_paddr);
            child_paddr = parent_paddr;
        }

        Ok(old_pa)
    }

    /// 只读遍历——从根向下查找叶 PTE，返回 PTE 及其所在层级。
    fn walk_readonly(&self, va: VirtAddr) -> Option<(PageTableEntry, usize)> {
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
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
        let table = unsafe { Table::from_paddr(paddr) };
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
    /// 失败时自动回滚已建立的映射，保证事务性。
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Errors
    ///
    /// 映射冲突或 `start >= end` 时返回错误。
    pub fn identity_map_range(
        &mut self,
        start: PhysAddr,
        end: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PageTableError> {
        let mut addr = start.align_down();
        let end_aligned = end.align_up();

        if addr.as_usize() >= end_aligned.as_usize() {
            return Err(PageTableError::InvalidRange);
        }

        // 第一阶段：收集所有 (va, pa, level) 映射
        let mut mappings: Vec<(VirtAddr, PhysAddr, usize)> = Vec::new();
        while addr.as_usize() < end_aligned.as_usize() {
            let remaining = end_aligned.as_usize() - addr.as_usize();
            let va = VirtAddr::new(addr.as_usize());

            let mut selected_level = 0;
            let mut selected_size = config::PAGE_SIZE;
            for level in (1..PT_LEVELS).rev() {
                let page_size = crate::page_size_at_level(level);
                if addr.as_usize().is_multiple_of(page_size) && remaining >= page_size {
                    selected_level = level;
                    selected_size = page_size;
                    break;
                }
            }
            mappings.push((va, addr, selected_level));
            addr += selected_size;
        }

        // 第二阶段：逐个映射，失败时回滚
        for (i, &(va, pa, level)) in mappings.iter().enumerate() {
            if let Err(e) = self.map_at_level(va, pa, flags, level) {
                for &(va, _, level) in mappings[..i].iter().rev() {
                    let _ = self.unmap_at_level(va, level);
                }
                return Err(e);
            }
        }
        Ok(())
    }
}
