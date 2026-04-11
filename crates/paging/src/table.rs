//! 多级页表——walk / set_page_flags / unmap 逻辑。

use core::sync::atomic::{AtomicU64, Ordering};

use alloc::collections::BTreeMap;
use memory_types::{PhysAddr, VirtAddr};

use frame_allocator::AllocatedFrames;

use crate::error::PagingError;
use crate::{ENTRIES_PER_TABLE, PageTableEntry, PteFlags, PteFlagsOps, PteOps, vpn_index};

const PT_LEVELS: usize = arch::PT_LEVELS;

/// 页表节点——封装 PTE 数组的原子访问。
///
/// 使用 `AtomicU64` 保证 SMP 下单个 PTE 读写不会 torn read/write。
/// 外层 `SpinLock` 负责更高层的互斥，此处仅保证单次访问的原子性。
struct Table {
    base: *mut AtomicU64,
}

impl Table {
    /// 从物理地址构造页表节点。
    ///
    /// # Safety
    /// - `paddr` 必须指向有效、页对齐的帧
    #[inline]
    unsafe fn from_paddr(paddr: PhysAddr) -> Self {
        Self {
            base: paddr.as_usize() as *mut AtomicU64,
        }
    }

    #[inline]
    fn read(&self, index: usize) -> PageTableEntry {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查。
        // Relaxed 即可——外层 SpinLock 提供必要的 memory barrier。
        let val = unsafe { (*self.base.add(index)).load(Ordering::Relaxed) };
        PageTableEntry::from_raw(val)
    }

    #[inline]
    fn write(&mut self, index: usize, pte: PageTableEntry) {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { (*self.base.add(index)).store(pte.as_raw(), Ordering::Relaxed) };
    }
}

/// 中间页表节点——持有帧所有权及有效 PTE 引用计数。
struct NodeEntry {
    /// 持有帧所有权——drop 时自动释放。
    /// 不直接访问帧内容（通过物理地址 + identity mapping 访问），
    /// 但必须持有所有权阻止帧被回收。
    _frame: AllocatedFrames,
    /// 该帧中有效 PTE 的数量。
    /// set_page_flags 时 +1，unmap 时 -1，count == 0 时可回收。
    ref_count: u16,
}

/// 多级页表。
///
/// 拥有根帧及所有遍历过程中分配的中间帧。
/// drop 时自动归还所有帧。
pub struct PageTable {
    /// 持有根帧所有权——通过 `.start_paddr()` 获取物理地址。
    root: AllocatedFrames,
    /// 根帧的有效 PTE 引用计数（根帧不在 nodes 中，单独记录）。
    root_ref_count: u16,
    /// 中间页表节点——以物理地址为键，O(log n) 查找/删除。
    nodes: BTreeMap<PhysAddr, NodeEntry>,
}

impl PageTable {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, PagingError> {
        let root = crate::alloc_node_frame()?;
        Ok(Self {
            root,
            root_ref_count: 0,
            nodes: BTreeMap::new(),
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root.start_paddr()
    }

    /// 获取指定帧的引用计数的可变引用。
    ///
    /// 根帧返回 `root_ref_count`，中间帧从 `nodes` 中查找。
    #[inline]
    fn ref_count_mut(&mut self, paddr: PhysAddr) -> &mut u16 {
        if paddr == self.root.start_paddr() {
            &mut self.root_ref_count
        } else {
            &mut self
                .nodes
                .get_mut(&paddr)
                .expect("ref_count_mut: 帧未注册")
                .ref_count
        }
    }

    /// 递增指定帧的引用计数。
    #[inline]
    fn inc_ref(&mut self, paddr: PhysAddr) {
        *self.ref_count_mut(paddr) += 1;
    }

    /// 递减指定帧的引用计数。
    #[inline]
    fn dec_ref(&mut self, paddr: PhysAddr) {
        *self.ref_count_mut(paddr) -= 1;
    }

    /// 映射用 walker——遍历到 Level 0 并按需分配中间节点。
    ///
    /// 返回目标 PTE 所在帧的物理地址及该 PTE 在帧中的索引。
    fn walk_create(&mut self, va: VirtAddr) -> Result<(PhysAddr, usize), PagingError> {
        let mut paddr = self.root.start_paddr();

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let mut table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = crate::alloc_node_frame()?;
                let frame_paddr = frame.start_paddr();
                // 先注册所有权，再写 PTE——若 BTreeMap::insert 因 OOM panic，
                // frame 随 NodeEntry drop 释放，但不会产生悬挂 PTE。
                self.nodes.insert(
                    frame_paddr,
                    NodeEntry {
                        _frame: frame,
                        ref_count: 0,
                    },
                );
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                self.inc_ref(paddr);
                paddr = frame_paddr;
            } else if pte.is_leaf(level) {
                panic!(
                    "walk_create: VA {} 在 level {} 遇到非预期的大页叶 PTE（页表损坏）",
                    va, level
                );
            } else {
                paddr = pte.paddr();
            }
        }

        let idx = vpn_index(va, 0);
        Ok((paddr, idx))
    }

    /// 设置单个虚拟页（4KB）的 PTE 标志位。
    ///
    /// SAS 全量映射下 PTE 始终存在，此方法的语义是：
    /// - 若该 VA 无 PTE → 创建 Level 0 叶 PTE
    /// - 若该 VA 已有 PTE 且 PA 相同 → 按需更新 flags
    /// - 若该 VA 已有 PTE 但 PA 不同 → panic（内核 bug）
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Panics
    ///
    /// - walk 路径上遇到非预期的大页叶 PTE（页表损坏）
    /// - VA 已映射到不同的 PA（内核 bug）
    pub fn set_page_flags(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PagingError> {
        let leaf_flags = flags.for_leaf_at_level(0);
        let (frame_paddr, idx) = self.walk_create(va)?;
        // SAFETY: frame_paddr 指向由 self 持有的有效帧
        let mut table = unsafe { Table::from_paddr(frame_paddr) };
        let current = table.read(idx);
        if current.is_valid() {
            if current.paddr() != pa {
                panic!(
                    "set_page_flags: VA {} 已指向 PA {}，试图改为 PA {}（不同 PA 是内核 bug）",
                    va,
                    current.paddr(),
                    pa
                );
            }
            // 同一 PA——幂等或权限变更，按需更新 flags
            // 注意：不调用 inc_ref，引用计数已在首次建立 PTE 时递增
            if current.flags() != leaf_flags {
                table.write(idx, PageTableEntry::new(pa, leaf_flags));
            }
            return Ok(());
        }
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
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, PagingError> {
        self.unmap_page_with_flags(va).map(|(pa, _)| pa)
    }

    /// 取消映射单个虚拟页（4KB），返回原始物理地址和 PTE 标志。
    ///
    /// unmap 后通过引用计数判断中间页表节点是否全空并回收，
    /// 避免遍历整个页表帧的 O(entries_per_table) 开销。
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Errors
    ///
    /// 目标 VA 未映射时返回 `PageNotMapped`。
    pub fn unmap_page_with_flags(
        &mut self,
        va: VirtAddr,
    ) -> Result<(PhysAddr, PteFlags), PagingError> {
        let mut path: [(PhysAddr, usize); PT_LEVELS] = [(PhysAddr::new(0), 0); PT_LEVELS];
        let mut path_len = 0;
        let mut paddr = self.root.start_paddr();

        for lv in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, lv);
            let pte = table.read(idx);
            if !pte.is_valid() || pte.is_leaf(lv) {
                return Err(PagingError::PageNotMapped);
            }
            let child_paddr = pte.paddr();
            path[path_len] = (paddr, idx);
            path_len += 1;
            paddr = child_paddr;
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { Table::from_paddr(paddr) };
        let idx = vpn_index(va, 0);
        let pte = table.read(idx);
        if !pte.is_valid() || !pte.is_leaf(0) {
            return Err(PagingError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        let old_flags = pte.flags();
        table.write(idx, PageTableEntry::empty());
        self.dec_ref(paddr);

        let mut child_paddr = paddr;
        for &(parent_paddr, parent_idx) in path[..path_len].iter().rev() {
            if *self.ref_count_mut(child_paddr) > 0 {
                break;
            }
            // SAFETY: parent_paddr 指向由 self 持有的有效帧
            let mut parent_table = unsafe { Table::from_paddr(parent_paddr) };
            parent_table.write(parent_idx, PageTableEntry::empty());
            self.nodes.remove(&child_paddr);
            self.dec_ref(parent_paddr);
            child_paddr = parent_paddr;
        }

        Ok((old_pa, old_flags))
    }

    /// 修改已映射页的权限标志位，保留物理地址不变。
    ///
    /// 单次页表遍历完成查找和更新，避免双重 walk 开销。
    ///
    /// **调用方必须在此操作后执行 TLB 刷新。**
    pub fn update_flags(
        &mut self,
        va: VirtAddr,
        new_flags: PteFlags,
    ) -> Result<PteFlags, PagingError> {
        let (pte, paddr, idx, leaf_level) =
            self.walk_to_leaf(va).ok_or(PagingError::PageNotMapped)?;

        let old_flags = pte.flags();
        let leaf_flags = new_flags.for_leaf_at_level(leaf_level);
        // SAFETY: paddr 指向叶 PTE 所在的帧
        let mut table = unsafe { Table::from_paddr(paddr) };
        table.write(idx, PageTableEntry::new(pte.paddr(), leaf_flags));

        Ok(old_flags)
    }

    /// 只读遍历——从根向下查找叶 PTE，返回已读取的 PTE、所在帧物理地址、索引及层级。
    ///
    /// 供 [`walk_to_leaf`] 和 [`update_flags`] 共用，避免重复 walk 逻辑。
    /// 返回已缓存的 PTE，调用方无需再次读取。
    fn walk_to_leaf(&self, va: VirtAddr) -> Option<(PageTableEntry, PhysAddr, usize, usize)> {
        let mut paddr = self.root.start_paddr();

        for level in (0..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return None;
            }
            if pte.is_leaf(level) {
                return Some((pte, paddr, idx, level));
            }
            paddr = pte.paddr();
        }

        None
    }

    /// 查询虚拟地址的映射信息，返回物理地址和标志。
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
        let (pte, _, _, level) = self.walk_to_leaf(va)?;
        let page_size = crate::page_size_at_level(level);
        let offset = va.as_usize() & (page_size - 1);
        Some((pte.paddr() + offset, pte.flags()))
    }

    /// 将 `[start, end)` 物理地址区间 identity-map（VA == PA），仅使用 4KB 页。
    ///
    /// ADR-006 移除了大页支持——SAS + QEMU 下大页无可观测收益。
    /// 所有页均以 4KB 粒度映射，调用 `set_page_flags`。
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Panics
    ///
    /// `start >= end` 或映射冲突时 panic。
    pub fn identity_map_range(&mut self, start: PhysAddr, end: PhysAddr, flags: PteFlags) {
        let mut addr = start.align_down();
        let end_aligned = end.align_up();

        assert!(
            addr.as_usize() < end_aligned.as_usize(),
            "identity_map_range: 无效地址范围 [{addr}, {end_aligned})"
        );

        while addr.as_usize() < end_aligned.as_usize() {
            let va = VirtAddr::new(addr.as_usize());
            match self.set_page_flags(va, addr, flags) {
                Ok(()) => {}
                Err(e) => panic!("identity_map_range: 设置 {va} 权限失败: {e}"),
            }
            addr += config::PAGE_SIZE;
        }
    }
}
