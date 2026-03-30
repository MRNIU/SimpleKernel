//! 多级页表——walk / map / unmap 逻辑。

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

#[cfg(test)]
mod tests {
    use crate::error::PageTableError;
    use crate::*;
    use address::{PhysAddr, VirtAddr};

    use crate::HeapNodeFrame;
    type PageTable = crate::table::PageTable<HeapNodeFrame>;

    /// create 后 root_paddr 应返回非零地址。
    #[test]
    fn root_paddr_is_valid() {
        let pt = PageTable::create().expect("创建测试页表失败");
        assert_ne!(pt.root_paddr().as_usize(), 0);
    }

    /// 映射单页后应能查询到正确的物理地址和完整标志。
    #[test]
    fn map_and_get_mapping() {
        let mut pt = PageTable::create().expect("创建测试页表失败");
        let va = VirtAddr::new(0x1000);
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PteFlags::kernel_rw();

        pt.map_page(va, pa, flags).expect("map_page 应成功");

        let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能找到映射");
        assert_eq!(mapped_pa, pa);
        assert_eq!(mapped_flags, flags);
    }

    /// 映射两个不同的虚拟页到不同的物理页，互不干扰。
    #[test]
    fn map_different_pages() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        let va1 = VirtAddr::new(0x0000_1000);
        let va2 = VirtAddr::new(0x0000_2000);
        let pa1 = PhysAddr::new(0x8020_0000);
        let pa2 = PhysAddr::new(0x8020_1000);

        pt.map_page(va1, pa1, PteFlags::kernel_rw())
            .expect("map va1");
        pt.map_page(va2, pa2, PteFlags::kernel_rx())
            .expect("map va2");

        let (got_pa1, got_flags1) = pt.get_mapping(va1).expect("va1 应已映射");
        let (got_pa2, got_flags2) = pt.get_mapping(va2).expect("va2 应已映射");
        assert_eq!(got_pa1, pa1);
        assert_eq!(got_pa2, pa2);
        assert_eq!(got_flags1, PteFlags::kernel_rw());
        assert_eq!(got_flags2, PteFlags::kernel_rx());
    }

    /// 对同一虚拟地址重复映射应返回 AlreadyMapped 错误。
    #[test]
    fn double_map_fails() {
        let mut pt = PageTable::create().expect("创建测试页表失败");
        let va = VirtAddr::new(0x1000);
        let pa = PhysAddr::new(0x8020_0000);

        pt.map_page(va, pa, PteFlags::kernel_rw())
            .expect("首次 map 应成功");
        let err = pt
            .map_page(va, pa, PteFlags::kernel_rw())
            .expect_err("重复 map 应失败");
        assert_eq!(err, PageTableError::AlreadyMapped);
    }

    /// unmap 应返回原始物理地址，且之后查询应为 None。
    #[test]
    fn unmap_page_returns_old_pa() {
        let mut pt = PageTable::create().expect("创建测试页表失败");
        let va = VirtAddr::new(0x1000);
        let pa = PhysAddr::new(0x8020_0000);

        pt.map_page(va, pa, PteFlags::kernel_rw())
            .expect("map 应成功");
        let old_pa = pt.unmap_page(va).expect("unmap 应成功");
        assert_eq!(old_pa, pa);

        assert!(pt.get_mapping(va).is_none());
    }

    /// 对未映射的页执行 unmap 应返回 PageNotMapped 错误。
    #[test]
    fn unmap_unmapped_page_fails() {
        let mut pt = PageTable::create().expect("创建测试页表失败");
        let va = VirtAddr::new(0x1000);

        let err = pt.unmap_page(va).expect_err("unmap 未映射页应失败");
        assert_eq!(err, PageTableError::PageNotMapped);
    }

    /// 跨不同 VPN[2] 范围的映射，会触发不同的二级页表分配。
    #[test]
    fn map_pages_in_different_vpn_ranges() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        let va_low = VirtAddr::new(0x0000_1000);
        let va_high = VirtAddr::new(0x4000_0000);
        let pa1 = PhysAddr::new(0x8020_0000);
        let pa2 = PhysAddr::new(0x8020_1000);

        pt.map_page(va_low, pa1, PteFlags::kernel_rw())
            .expect("map low");
        pt.map_page(va_high, pa2, PteFlags::kernel_rw())
            .expect("map high");

        let (got1, _) = pt.get_mapping(va_low).expect("low 应已映射");
        let (got2, _) = pt.get_mapping(va_high).expect("high 应已映射");
        assert_eq!(got1, pa1);
        assert_eq!(got2, pa2);
    }

    /// unmap 后重新映射到不同物理地址应成功。
    #[test]
    fn remap_after_unmap() {
        let mut pt = PageTable::create().expect("创建测试页表失败");
        let va = VirtAddr::new(0x1000);
        let pa1 = PhysAddr::new(0x8020_0000);
        let pa2 = PhysAddr::new(0x8030_0000);

        pt.map_page(va, pa1, PteFlags::kernel_rw())
            .expect("首次 map");
        pt.unmap_page(va).expect("unmap");
        pt.map_page(va, pa2, PteFlags::kernel_rx())
            .expect("重映射应成功");

        let (got_pa, got_flags) = pt.get_mapping(va).expect("应找到新映射");
        assert_eq!(got_pa, pa2);
        assert_eq!(got_flags, PteFlags::kernel_rx());
    }

    /// 查询从未映射过的地址应返回 None。
    #[test]
    fn get_mapping_on_empty_table() {
        let pt = PageTable::create().expect("创建测试页表失败");
        assert!(pt.get_mapping(VirtAddr::new(0x1000)).is_none());
        assert!(pt.get_mapping(VirtAddr::new(0)).is_none());
    }

    /// unmap 唯一叶后，中间节点也被回收——同路径上的其他 VA unmap 应返回 PageNotMapped。
    #[test]
    fn unmap_reclaims_intermediate_then_sibling_fails() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        let va = VirtAddr::new(0x1000);
        let pa = PhysAddr::new(0x8020_0000);
        pt.map_page(va, pa, PteFlags::kernel_rw())
            .expect("map 应成功");
        pt.unmap_page(va).expect("unmap 应成功");

        // 中间节点已回收，同路径的兄弟 VA 也不可达
        let va_sibling = VirtAddr::new(0x2000);
        let err = pt
            .unmap_page(va_sibling)
            .expect_err("中间节点已回收，应返回 PageNotMapped");
        assert_eq!(err, PageTableError::PageNotMapped);
    }

    /// unmap 后中间节点回收：当同表有其他映射时不回收。
    #[test]
    fn unmap_preserves_intermediate_when_sibling_exists() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        let va1 = VirtAddr::new(0x1000);
        let va2 = VirtAddr::new(0x2000);
        let pa1 = PhysAddr::new(0x8020_0000);
        let pa2 = PhysAddr::new(0x8020_1000);

        pt.map_page(va1, pa1, PteFlags::kernel_rw())
            .expect("map va1");
        pt.map_page(va2, pa2, PteFlags::kernel_rw())
            .expect("map va2");

        // unmap va1，va2 仍在同一中间节点中——中间节点不应被回收
        pt.unmap_page(va1).expect("unmap va1");
        assert!(pt.get_mapping(va1).is_none(), "va1 应已 unmap");
        assert!(pt.get_mapping(va2).is_some(), "va2 应仍然有效");

        // unmap va2 后可重新映射（中间节点此时回收，重新分配）
        pt.unmap_page(va2).expect("unmap va2");
        pt.map_page(va1, pa1, PteFlags::kernel_rw())
            .expect("重映射应成功");
    }

    /// identity_map_range 多页映射后应能逐页查询。
    #[test]
    fn identity_map_range_multi_page() {
        let mut pt = PageTable::create().expect("创建测试页表失败");
        let start = PhysAddr::new(0x10_0000);
        let end = PhysAddr::new(0x10_3000); // 3 pages

        pt.identity_map_range(start, end, PteFlags::kernel_rw())
            .expect("identity_map_range 应成功");

        for i in 0..3 {
            let va = VirtAddr::new(0x10_0000 + i * config::PAGE_SIZE);
            let (pa, _) = pt.get_mapping(va).expect("应能查到映射");
            assert_eq!(pa, PhysAddr::new(0x10_0000 + i * config::PAGE_SIZE));
        }
    }

    /// 大页映射：Level 1（2MB）应能映射和查询。
    #[test]
    fn map_at_level1_huge_page() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        let huge_size = page_size_at_level(1);
        let va = VirtAddr::new(huge_size);
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PteFlags::kernel_rw();

        pt.map_at_level(va, pa, flags, 1).expect("大页映射应成功");

        let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能查询到大页映射");
        assert_eq!(mapped_pa, pa);
        assert_eq!(mapped_flags, flags.for_leaf_at_level(1));
    }

    /// 大页范围内不同偏移处的 VA 应返回精确物理地址（基址 + 页内偏移）。
    #[test]
    fn get_mapping_within_huge_page() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        let huge_size = page_size_at_level(1);
        let va_base = VirtAddr::new(huge_size);
        let pa = PhysAddr::new(0x8020_0000);

        pt.map_at_level(va_base, pa, PteFlags::kernel_rw(), 1)
            .expect("大页映射应成功");

        // 基地址查询——偏移 0
        let (got_pa_base, _) = pt.get_mapping(va_base).expect("大页基地址应命中映射");
        assert_eq!(got_pa_base, pa);

        // 偏移地址查询——应返回 pa + 0x1000
        let va_offset = VirtAddr::new(huge_size + 0x1000);
        let (got_pa, _) = pt.get_mapping(va_offset).expect("大页内偏移地址应命中映射");
        assert_eq!(got_pa, pa + 0x1000);
    }

    /// 大页映射后，不可在同一路径上再映射子页（路径上遇到大页返回 HugePageConflict）。
    #[test]
    fn map_page_under_huge_page_fails() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        let huge_size = page_size_at_level(1);
        let va = VirtAddr::new(huge_size);
        let pa = PhysAddr::new(0x8020_0000);

        pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
            .expect("大页映射应成功");

        let sub_va = VirtAddr::new(huge_size + 0x1000);
        let err = pt
            .map_page(sub_va, PhysAddr::new(0x9000_0000), PteFlags::kernel_rw())
            .expect_err("大页范围内的子映射应失败");
        assert_eq!(err, PageTableError::HugePageConflict);
    }

    /// 重复大页映射应返回 AlreadyMapped。
    #[test]
    fn double_map_at_level_fails() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        let huge_size = page_size_at_level(1);
        let va = VirtAddr::new(huge_size);
        let pa = PhysAddr::new(0x8020_0000);

        pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
            .expect("首次大页映射应成功");
        let err = pt
            .map_at_level(va, pa, PteFlags::kernel_rw(), 1)
            .expect_err("重复大页映射应失败");
        assert_eq!(err, PageTableError::AlreadyMapped);
    }

    /// unmap_at_level 应能取消大页映射。
    #[test]
    fn unmap_at_level1_huge_page() {
        let mut pt = PageTable::create().expect("创建页表");
        let va = VirtAddr::new(0x0000_0000_4000_0000); // 1GB aligned
        let pa = PhysAddr::new(0x0000_0000_4000_0000);
        pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
            .expect("map level1");
        let old_pa = pt.unmap_at_level(va, 1).expect("unmap level1 应成功");
        assert_eq!(old_pa, pa);
        assert!(pt.get_mapping(va).is_none(), "unmap 后应无映射");
    }

    /// unmap_at_level 目标层级无叶节点时应失败。
    #[test]
    fn unmap_at_level_wrong_level_fails() {
        let mut pt = PageTable::create().expect("创建页表");
        let va = VirtAddr::new(0x0000_0000_4000_0000);
        let pa = PhysAddr::new(0x0000_0000_4000_0000);
        // 在 level 1 映射，尝试在 level 0 unmap 应失败
        pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
            .expect("map level1");
        let err = pt
            .unmap_at_level(va, 0)
            .expect_err("level 0 unmap 大页应失败");
        assert_eq!(err, PageTableError::PageNotMapped);
    }

    /// identity_map_range 对无效范围（start >= end）应返回 InvalidRange。
    #[test]
    fn identity_map_range_invalid_range() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        let err = pt
            .identity_map_range(
                PhysAddr::new(0x10_0000),
                PhysAddr::new(0x10_0000),
                PteFlags::kernel_rw(),
            )
            .expect_err("start == end 应返回 InvalidRange");
        assert_eq!(err, PageTableError::InvalidRange);

        let err = pt
            .identity_map_range(
                PhysAddr::new(0x20_0000),
                PhysAddr::new(0x10_0000),
                PteFlags::kernel_rw(),
            )
            .expect_err("start > end 应返回 InvalidRange");
        assert_eq!(err, PageTableError::InvalidRange);
    }

    /// identity_map_range 在对齐且足够大的区间应自动使用大页。
    #[test]
    fn identity_map_range_auto_huge_page() {
        let mut pt = PageTable::create().expect("创建测试页表失败");
        let huge_size = page_size_at_level(1); // 2MB
        let start = PhysAddr::new(huge_size);
        let end = PhysAddr::new(huge_size * 2);

        pt.identity_map_range(start, end, PteFlags::kernel_rw())
            .expect("identity_map_range 应成功");

        // 大页基址应能查询到映射
        let (pa, flags) = pt
            .get_mapping(VirtAddr::new(huge_size))
            .expect("大页基址应已映射");
        assert_eq!(pa, start);
        assert_eq!(flags, PteFlags::kernel_rw().for_leaf_at_level(1));

        // 大页内偏移地址也应命中
        let (pa_offset, _) = pt
            .get_mapping(VirtAddr::new(huge_size + 0x1000))
            .expect("大页内偏移应命中");
        assert_eq!(pa_offset, PhysAddr::new(huge_size + 0x1000));
    }

    /// identity_map_range 失败时应回滚已建立的映射。
    #[test]
    fn identity_map_range_rollback_on_conflict() {
        let mut pt = PageTable::create().expect("创建测试页表失败");

        // 先占住一个页，使后续 identity_map_range 在映射到该地址时冲突
        let conflict_va = VirtAddr::new(0x10_2000);
        let conflict_pa = PhysAddr::new(0x10_2000);
        pt.map_page(conflict_va, conflict_pa, PteFlags::kernel_rw())
            .expect("占位映射应成功");

        // identity_map_range 试图映射 [0x10_0000, 0x10_3000)，在第 3 页会冲突
        let start = PhysAddr::new(0x10_0000);
        let end = PhysAddr::new(0x10_3000);
        pt.identity_map_range(start, end, PteFlags::kernel_rw())
            .expect_err("应因冲突而失败");

        // 前两页应已回滚，查询应为 None
        assert!(
            pt.get_mapping(VirtAddr::new(0x10_0000)).is_none(),
            "回滚后第 1 页不应存在"
        );
        assert!(
            pt.get_mapping(VirtAddr::new(0x10_1000)).is_none(),
            "回滚后第 2 页不应存在"
        );

        // 原始占位映射应保持不变
        let (pa, _) = pt.get_mapping(conflict_va).expect("占位映射应仍存在");
        assert_eq!(pa, conflict_pa);
    }
}
