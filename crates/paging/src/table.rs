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
/// `Acquire/Release` ordering 保证：
/// - `read`（Acquire）：读到的 PTE 值与写入方的所有先序写操作同步
/// - `write`（Release）：写入 PTE 前的所有先序写操作对后续 Acquire 读可见
/// - `swap`（AcqRel）：兼具 Acquire + Release 语义，用于无锁 update_pte
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
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        let val = unsafe { (*self.base.add(index)).load(Ordering::Acquire) };
        PageTableEntry::from_raw(val)
    }

    #[inline]
    fn write(&self, index: usize, pte: PageTableEntry) {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { (*self.base.add(index)).store(pte.as_raw(), Ordering::Release) };
    }

    /// 原子交换 PTE，返回旧值。用于无锁 update_pte。
    #[inline]
    fn swap(&self, index: usize, pte: PageTableEntry) -> PageTableEntry {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        let old = unsafe { (*self.base.add(index)).swap(pte.as_raw(), Ordering::AcqRel) };
        PageTableEntry::from_raw(old)
    }
}

/// 多级页表——无锁 hot path + 内部锁保护中间节点分配。
///
/// SAS 全量映射下 PTE 只建不删——中间节点一旦建立永久有效，
/// 无 dangling 引用风险。这让无锁 walk 和原子 PTE 更新成为可能。
///
/// 锁粒度：
/// - `root`：创建后永不变动，无需同步保护
/// - `nodes`：仅 `create_pte` 的慢路径（建新 PTE）需要互斥
/// - 单个 PTE：`AtomicU64` per-entry，`Acquire/Release` ordering
pub struct PageTable {
    root: AllocatedFrames,
    /// 中间节点容器——仅 `create_pte` 的慢路径需要锁。
    nodes: sync_crate::SpinLock<Vec<AllocatedFrames>>,
}

impl PageTable {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, PagingError> {
        let root = crate::alloc_node_frame()?;
        Ok(Self {
            root,
            nodes: sync_crate::SpinLock::new(
                Vec::new(),
                "pt_nodes",
                sync_crate::lock_level::KERNEL_PT,
            ),
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root.start_paddr()
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
        &self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PagingError> {
        let leaf_flags = flags.for_leaf_at_level(0);

        // 快速路径：先无锁检查 PTE 是否已存在
        if let Some((pte, _, _, level)) = self.walk_to_leaf(va) {
            if pte.paddr() != pa {
                panic!(
                    "create_pte: VA {} 已指向 PA {}，试图改为 PA {}（不同 PA 是内核 bug）",
                    va,
                    pte.paddr(),
                    pa
                );
            }
            let existing_leaf_flags = flags.for_leaf_at_level(level);
            if pte.flags() == existing_leaf_flags {
                return Ok(()); // 幂等
            }
            return Err(PagingError::FlagsConflict);
        }

        // 慢路径：PTE 不存在，需要分配中间节点——取锁
        let mut nodes = self.nodes.lock();
        self.walk_create_and_write(&mut nodes, va, pa, leaf_flags)
    }

    /// 持锁建立新 PTE——walk 到 Level 0 并按需分配中间节点，写入叶 PTE。
    ///
    /// TOCTOU re-check：快速路径到取锁之间，另一核可能已建立该 PTE。
    fn walk_create_and_write(
        &self,
        nodes: &mut Vec<AllocatedFrames>,
        va: VirtAddr,
        pa: PhysAddr,
        leaf_flags: PteFlags,
    ) -> Result<(), PagingError> {
        let mut paddr = self.root.start_paddr();

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = crate::alloc_node_frame()?;
                let frame_paddr = frame.start_paddr();
                nodes.push(frame);
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
        // SAFETY: paddr 指向由 self 持有的有效帧
        let table = unsafe { Table::from_paddr(paddr) };
        let current = table.read(idx);

        // TOCTOU re-check：另一核可能在我们取锁期间已建立该 PTE
        if current.is_valid() {
            if current.paddr() != pa {
                panic!(
                    "create_pte: VA {} 已指向 PA {}，试图改为 PA {}（不同 PA 是内核 bug）",
                    va,
                    current.paddr(),
                    pa
                );
            }
            if current.flags() == leaf_flags {
                return Ok(()); // 幂等
            }
            return Err(PagingError::FlagsConflict);
        }

        table.write(idx, PageTableEntry::new(pa, leaf_flags));
        Ok(())
    }

    /// 修改已映射页的权限标志位，保留物理地址不变。
    ///
    /// 无锁操作——使用原子 swap 替换叶 PTE。
    ///
    /// **调用方必须在此操作后执行 TLB 刷新。**
    pub fn update_pte(&self, va: VirtAddr, new_flags: PteFlags) -> Result<PteFlags, PagingError> {
        let (pte, paddr, idx, leaf_level) =
            self.walk_to_leaf(va).ok_or(PagingError::PageNotMapped)?;

        let leaf_flags = new_flags.for_leaf_at_level(leaf_level);
        let new_pte = PageTableEntry::new(pte.paddr(), leaf_flags);
        // SAFETY: paddr 源自 self.root 或 self.nodes 持有的帧——生命周期由 &self 保证
        let table = unsafe { Table::from_paddr(paddr) };
        let old = table.swap(idx, new_pte);

        Ok(PageTableEntry::from_raw(old.as_raw()).flags())
    }

    /// 只读遍历——从根向下查找叶 PTE，返回已读取的 PTE、所在帧物理地址、索引及层级。
    ///
    /// 无锁操作——SAS 下中间节点只建不删，walk 路径稳定。
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
    /// 所有页均以 4KB 粒度映射。
    ///
    /// 批量操作——内部只取一次 nodes 锁，避免逐页锁获取/释放开销。
    /// 对 128 MB RAM（32K 页）从 32K 次锁操作降为 1 次。
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新**
    /// （RISC-V: `sfence.vma`，AArch64: `TLBI` + `DSB` + `ISB`）。
    ///
    /// # Panics
    ///
    /// `start >= end` 或映射冲突时 panic。
    pub fn identity_map_range(&self, start: PhysAddr, end: PhysAddr, flags: PteFlags) {
        let mut addr = start.align_down();
        let end_aligned = end.align_up();

        assert!(
            addr.as_usize() < end_aligned.as_usize(),
            "identity_map_range: 无效地址范围 [{addr}, {end_aligned})"
        );

        let leaf_flags = flags.for_leaf_at_level(0);
        let mut nodes = self.nodes.lock();

        while addr.as_usize() < end_aligned.as_usize() {
            let va = VirtAddr::new(addr.as_usize());
            match self.walk_create_and_write(&mut nodes, va, addr, leaf_flags) {
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
