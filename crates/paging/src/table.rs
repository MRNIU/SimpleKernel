//! 多级页表——walk / create_pte / update_pte 逻辑。

use core::sync::atomic::{AtomicU64, Ordering};

use alloc::vec::Vec;
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

/// 多级页表。
///
/// 拥有根帧及所有遍历过程中分配的中间帧。
/// drop 时自动归还所有帧。
pub struct PageTable {
    /// 持有根帧所有权——通过 `.start_paddr()` 获取物理地址。
    root: AllocatedFrames,
    /// 中间页表节点——仅持有所有权，drop 时自动释放。
    nodes: Vec<AllocatedFrames>,
}

impl PageTable {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, PagingError> {
        let root = crate::alloc_node_frame()?;
        Ok(Self {
            root,
            nodes: Vec::new(),
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root.start_paddr()
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
                // 先注册所有权，再写 PTE——若 Vec::push 因 OOM panic，
                // frame 随 drop 释放，但不会产生悬挂 PTE。
                self.nodes.push(frame);
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
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

    /// 创建单个虚拟页（4KB）的 PTE。
    ///
    /// SAS 全量映射下 PTE 始终存在。此方法的语义：
    /// - 若该 VA 无 PTE → 创建 Level 0 叶 PTE，返回 Ok
    /// - 若该 VA 已有 PTE 且 PA 相同且 flags 相同 → 幂等，返回 Ok
    /// - 若该 VA 已有 PTE 且 PA 相同但 flags 不同 → Err(FlagsConflict)；
    ///   显式修改 flags 请使用 [`update_pte`](Self::update_pte)
    /// - 若该 VA 已有 PTE 但 PA 不同 → panic（内核 bug）
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Errors
    ///
    /// - `PagingError::AllocationFailed` — 中间节点帧分配失败
    /// - `PagingError::FlagsConflict` — 同 PA 不同 flags
    ///
    /// # Panics
    ///
    /// - walk 路径上遇到非预期的大页叶 PTE（页表损坏）
    /// - VA 已映射到不同的 PA（内核 bug）
    pub fn create_pte(
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
                    "create_pte: VA {} 已指向 PA {}，试图改为 PA {}（不同 PA 是内核 bug）",
                    va,
                    current.paddr(),
                    pa
                );
            }
            if current.flags() != leaf_flags {
                return Err(PagingError::FlagsConflict);
            }
            return Ok(());
        }
        table.write(idx, PageTableEntry::new(pa, leaf_flags));
        Ok(())
    }

    /// 修改已映射页的权限标志位，保留物理地址不变。
    ///
    /// 单次页表遍历完成查找和更新，避免双重 walk 开销。
    ///
    /// **调用方必须在此操作后执行 TLB 刷新。**
    pub fn update_pte(
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
    /// 供 [`walk_to_leaf`] 和 [`update_pte`] 共用，避免重复 walk 逻辑。
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
    /// 所有页均以 4KB 粒度映射，调用 `create_pte`。
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
            match self.create_pte(va, addr, flags) {
                Ok(()) => {}
                Err(PagingError::FlagsConflict) => panic!(
                    "identity_map_range: VA {} flags 冲突——已有 PTE 的权限与请求不同",
                    va
                ),
                Err(e) => panic!("identity_map_range: 设置 {va} 权限失败: {e}"),
            }
            addr += config::PAGE_SIZE;
        }
    }
}
