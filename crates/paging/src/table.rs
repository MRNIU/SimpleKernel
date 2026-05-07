//! 多级页表——walk / identity_map_range / update_range_flags 逻辑。

use core::sync::atomic::{AtomicU64, Ordering};

use alloc::vec::Vec;
use memory_types::{PhysAddr, VirtAddr};

use frame_allocator::AllocatedFrames;

use crate::{ENTRIES_PER_TABLE, PageTableEntry, PteFlags, PteFlagsOps, PteOps, vpn_index};

const PT_LEVELS: usize = arch::PT_LEVELS;

/// 页表节点——封装 PTE 数组的原子访问。
///
/// 使用 `AtomicU64` 保证 SMP 下单个 PTE 读写不会 torn read/write。
/// `Acquire/Release` ordering 保证：
/// - `read`（Acquire）：读到的 PTE 值与写入方的所有先序写操作同步
/// - `write`（Release）：写入 PTE 前的所有先序写操作对后续 Acquire 读可见
/// - `swap`（AcqRel）：兼具 Acquire + Release 语义，用于内部 PTE 覆盖
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
            base: paddr.to_virt().as_mut_ptr::<AtomicU64>(),
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

    /// 原子交换 PTE，返回旧值。用于无锁权限覆盖。
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
/// - `nodes`：仅建新 PTE 的慢路径需要互斥（`identity_map_range`）
/// - 单个 PTE：`AtomicU64` per-entry，`Acquire/Release` ordering
pub struct PageTable {
    root: AllocatedFrames,
    nodes: sync_crate::SpinLock<Vec<AllocatedFrames>>,
}

impl PageTable {
    /// 创建新页表，分配根帧。
    pub fn create() -> Self {
        let root = crate::alloc_node_frame();
        Self {
            root,
            nodes: sync_crate::SpinLock::new(
                Vec::new(),
                "pt_nodes",
                sync_crate::lock_level::KERNEL_PT,
            ),
        }
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root.start_paddr()
    }

    /// 持锁 walk 到 Level 0 并按需分配中间节点，写入叶 PTE。
    ///
    /// 若该 VA 已存在 PTE：
    /// - 同 PA 同 flags → 幂等，直接返回
    /// - 其他情况（不同 PA 或不同 flags）→ panic（调用方 bug）
    fn walk_create_and_write(
        &self,
        nodes: &mut Vec<AllocatedFrames>,
        va: VirtAddr,
        pa: PhysAddr,
        leaf_flags: PteFlags,
    ) {
        let mut paddr = self.root.start_paddr();

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = crate::alloc_node_frame();
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
            assert_eq!(
                current.paddr(),
                pa,
                "identity_map_range: VA {va} 已指向 PA {}，试图改为 PA {pa}（内核 bug）",
                current.paddr(),
            );
            assert_eq!(
                current.flags(),
                leaf_flags,
                "identity_map_range: VA {va} flags 冲突——已有 PTE {:?} 与请求 {leaf_flags:?} 不同",
                current.flags(),
            );
            return; // 幂等重复映射
        }

        table.write(idx, PageTableEntry::new(pa, leaf_flags));
    }

    /// 修改已映射页的权限标志位，保留物理地址不变。
    ///
    /// 无锁操作——使用原子 swap 替换叶 PTE。
    ///
    /// **调用方必须在此操作后执行 TLB 刷新。**
    ///
    /// # Panics
    ///
    /// 目标 VA 未映射时 panic——SAS 架构下所有物理内存都有背景 identity mapping，
    /// 未映射是违反不变量的内核 bug。
    ///
    /// # Safety
    ///
    /// 调用方必须保证：
    /// - 同一 PTE 没有并发写入者；
    /// - 修改完成后在重新依赖权限语义前刷新本核和其他在线核心的 TLB；
    /// - 权限收紧或释放帧前，调用方不能让旧权限继续被观察。
    unsafe fn update_pte(&self, va: VirtAddr, new_flags: PteFlags) {
        let (pte, paddr, idx, leaf_level) = self
            .walk_to_leaf(va)
            .unwrap_or_else(|| panic!("update_pte: VA {va} 未映射（违反 SAS 背景层不变量）"));

        let leaf_flags = new_flags.for_leaf_at_level(leaf_level);
        let new_pte = PageTableEntry::new(pte.paddr(), leaf_flags);
        // SAFETY: paddr 源自 self.root 或 self.nodes 持有的帧——生命周期由 &self 保证
        let table = unsafe { Table::from_paddr(paddr) };
        table.swap(idx, new_pte);
    }

    /// 批量修改 `[va_start, va_start + page_count * PAGE_SIZE)` 的权限位并刷新 TLB。
    ///
    /// 每页通过 [`update_pte`](Self::update_pte) 独立更新，最后由 `TlbFlushGuard`
    /// 在 drop 时统一刷 TLB——避免中途过期的 TLB 条目被其他核观察到。
    ///
    /// 参数用 `(va_start, page_count)` 而非 byte range——调用方（DMA 分配器、未来
    /// `mprotect`）天然以页为单位持有数量。对照 [`identity_map_range`](Self::identity_map_range)
    /// 接收 byte range。
    ///
    /// # 跨页非原子
    ///
    /// 区间内各页**独立**更新——更新第 `k+1` 页时第 `k` 页已写回，其他核可能观察到
    /// 前半区间新 flags、后半区间旧 flags 的**中间状态**。若调用场景要求整段翻转
    /// 不可分（如 W^X 切换），调用方需自行在更高层加锁或使用 stop-the-world 协议。
    ///
    /// # 并发写者
    ///
    /// 当前生产路径只在启动期 `memory::init()` 覆盖内核段和固件保留区权限，
    /// 此时 SMP 尚未进入运行期调度，不存在多核同时修改同一 VA 的调用路径。
    /// 后续若把此接口用于运行期 DMA 属性切换、模块加载或 `mprotect`，必须先在
    /// 上层引入区间锁、owner token 或等价的地址空间所有权证明。
    ///
    /// # Panics
    ///
    /// 区间内任一页未映射时 panic（同 [`update_pte`](Self::update_pte)）。
    //
    // TODO: 单次 walk 批量更新——当前每页独立调用 `update_pte` 触发完整 walk，
    // 同一区间的页通常共享中间节点，可合并为单次 walk 后按叶节点索引步进。
    // 待引入大映射场景（mmap / 模块加载）后优化。
    pub fn update_range_flags(&self, va_start: VirtAddr, page_count: usize, flags: PteFlags) {
        for i in 0..page_count {
            // SAFETY: update_range_flags 是页表权限覆盖的受控入口；循环内按页串行更新，
            // 同一调用不会并发写同一 PTE，函数末尾用 TlbFlushGuard 刷新所有目标页。
            unsafe { self.update_pte(va_start + i * config::PAGE_SIZE, flags) };
        }
        let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
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
    /// 参数用 byte range——调用方（FDT / MMIO 描述符）天然持有字节起止，内部
    /// `align_down`/`align_up` 吸收对齐差异。对照 [`update_range_flags`](Self::update_range_flags)
    /// 接收 `(va, page_count)`。
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
    /// `start >= end`、映射冲突（同 PA 不同 flags）、页表节点 OOM 时 panic。
    pub fn identity_map_range(&self, start: PhysAddr, end: PhysAddr, flags: PteFlags) {
        let addr_start = start.align_down();
        let end_aligned = end.align_up();

        assert!(
            addr_start.as_usize() < end_aligned.as_usize(),
            "identity_map_range: 无效地址范围 [{addr_start}, {end_aligned})"
        );

        let leaf_flags = flags.for_leaf_at_level(0);
        let mut nodes = self.nodes.lock();

        let mut addr = addr_start;
        while addr.as_usize() < end_aligned.as_usize() {
            self.walk_create_and_write(&mut nodes, addr.to_virt(), addr, leaf_flags);
            addr += config::PAGE_SIZE;
        }
    }
}
