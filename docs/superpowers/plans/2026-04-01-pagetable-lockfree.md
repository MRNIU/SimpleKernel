# PageTable 完全无锁重构实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除 PageTable 的所有锁——map 用 CAS，unmap 用 atomic swap，中间节点所有权编码到 PTE，PageTable::drop 递归 walk 回收。

**Architecture:** 外部接口从 `Arc<SpinLock<PageTable>>` 简化为 `Arc<PageTable>`。PTE 操作全部通过 `AtomicU64` 的 `Acquire load` / `AcqRel swap` / `AcqRel CAS` 完成。中间节点分配后 `forget`，所有权编码在 PTE 中，`PageTable::drop` 递归遍历整棵树回收。`MappedPagesInner` 删除 `flags`/`exclusive` 缓存字段，PTE 是唯一 source of truth。

**Tech Stack:** Rust nightly, `#![no_std]`, `AtomicU64`, `heapless`

**Spec:** `docs/superpowers/specs/2026-04-01-pagetable-internal-lock-design.md`

---

## 文件变更总览

| 文件 | 动作 | 说明 |
|------|------|------|
| `crates/paging/src/lib.rs` | 修改 | Table 原子操作 + NodeFrameOps::reclaim + test_pt 返回类型 |
| `crates/paging/src/table.rs` | 重构 | PageTable 结构拆分 + CAS map + atomic unmap + Drop walk |
| `crates/paging/src/mapping.rs` | 修改 | MappedPagesInner 精简 + unmap_and_reclaim 无锁化 |
| `crates/paging/src/mmio.rs` | 修改 | map_to 签名 |
| `crates/memory/src/vma.rs` | 修改 | AddressSpace 字段 + 签名 |
| `crates/memory/src/globals.rs` | 修改 | KERNEL_PAGE_TABLE 类型 |
| `crates/memory/src/init.rs` | 修改 | init_smp 删 lock |
| `crates/memory/src/lib.rs` | 修改 | 删 SpinLock import |
| `src/main.rs` | 修改 | 删 lock() |
| `src/boot.rs` | 修改 | 删 lock() |

---

### Task 1: Table 原子操作——替换 read/write 为 Acquire/CAS/swap

**Files:**
- Modify: `crates/paging/src/lib.rs:172-203`

- [ ] **Step 1: 替换 Table::read 为 read_acquire，删除 write，新增 swap 和 compare_exchange**

将 `Table` 的方法从 Relaxed read / Relaxed write 替换为无锁原子操作。所有方法均为 `&self`。

```rust
impl Table {
    /// 从物理地址构造页表节点。
    ///
    /// # Safety
    /// - `paddr` 必须指向有效、页对齐的帧
    #[inline]
    pub(crate) unsafe fn from_paddr(paddr: address::PhysAddr) -> Self {
        Self {
            base: paddr.as_usize() as *mut AtomicU64,
        }
    }

    /// Acquire load——无锁 walk 路径。
    #[inline]
    pub(crate) fn read_acquire(&self, index: usize) -> PageTableEntry {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        let val = unsafe { (*self.base.add(index)).load(Ordering::Acquire) };
        PageTableEntry::from_raw(val)
    }

    /// AcqRel swap——无锁 unmap（原子清零叶 PTE）。
    #[inline]
    pub(crate) fn swap(&self, index: usize, val: u64) -> u64 {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { (*self.base.add(index)).swap(val, Ordering::AcqRel) }
    }

    /// AcqRel CAS——无锁 map（安装中间节点或叶 PTE）。
    ///
    /// 返回 `Result`：`Ok(old)` CAS 成功，`Err(actual)` CAS 失败。
    /// 与 `std::sync::atomic::AtomicU64::compare_exchange` 语义一致。
    #[inline]
    pub(crate) fn compare_exchange(&self, index: usize, expected: u64, new: u64) -> Result<u64, u64> {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe {
            (*self.base.add(index)).compare_exchange(expected, new, Ordering::AcqRel, Ordering::Acquire)
        }
    }
}
```

- [ ] **Step 2: 运行 paging crate 测试（预期部分失败——table.rs 仍使用旧 API）**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging 2>&1 | tail -30`
Expected: 编译失败——`table.rs` 中 `table.read()` 和 `table.write()` 调用不存在

- [ ] **Step 3: Commit**

```bash
git add crates/paging/src/lib.rs
git commit --signoff -m "refactor(paging): Table 替换为 Acquire/CAS/swap 原子操作"
```

---

### Task 2: NodeFrameOps 扩展——新增 reclaim 方法

**Files:**
- Modify: `crates/paging/src/lib.rs:52-128`

- [ ] **Step 1: 在 NodeFrameOps trait 中新增 reclaim 方法**

在 `NodeFrameOps` trait 定义中新增 `reclaim`，并在 `KernelNodeFrame` 和 `HeapNodeFrame` 上实现：

```rust
pub trait NodeFrameOps: Send + Sized {
    /// 分配一个零初始化的页表节点帧。
    fn alloc() -> Result<Self, error::PagingError>;
    /// 获取帧的物理地址（裸机 identity mapping）或堆地址（测试）。
    fn paddr(&self) -> PhysAddr;

    /// 从物理地址回收帧——仅在 PageTable::drop 中使用。
    ///
    /// # Safety
    ///
    /// `paddr` 必须是本 trait 的 `alloc()` 分配、尚未释放的帧，
    /// 且调用方保证无并发访问。
    unsafe fn reclaim(paddr: PhysAddr);
}
```

KernelNodeFrame 实现（裸机）：

```rust
#[cfg(target_os = "none")]
impl NodeFrameOps for KernelNodeFrame {
    // alloc 和 paddr 保持不变...

    unsafe fn reclaim(paddr: PhysAddr) {
        use address::{FrameRange, PhysPageNum};
        let ppn = PhysPageNum::from(paddr);
        let range = FrameRange::new(ppn, ppn + 1);
        // SAFETY: 调用方保证帧由 alloc() 分配且未释放，无并发访问。
        let _reclaimed = unsafe { frame_allocator::UnmappedFrames::from_range(range) };
    }
}
```

HeapNodeFrame 实现（测试）：

```rust
#[cfg(any(test, feature = "test-support"))]
impl NodeFrameOps for HeapNodeFrame {
    // alloc 和 paddr 保持不变...

    unsafe fn reclaim(paddr: PhysAddr) {
        let ptr = paddr.as_usize() as *mut u8;
        // layout 必须与 alloc() 中的 Layout::from_size_align(PAGE_SIZE, PAGE_SIZE) 一致
        let layout = core::alloc::Layout::from_size_align(config::PAGE_SIZE, config::PAGE_SIZE)
            .expect("HeapNodeFrame::reclaim: invalid layout");
        // SAFETY: ptr 由同 layout 的 alloc_zeroed 分配，调用方保证未释放
        unsafe { alloc::alloc::dealloc(ptr, layout) };
    }
}
```

- [ ] **Step 2: 运行 paging crate 测试（预期编译失败——table.rs 仍使用旧 API）**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging 2>&1 | tail -30`
Expected: 编译失败（与 Task 1 相同原因）

- [ ] **Step 3: Commit**

```bash
git add crates/paging/src/lib.rs
git commit --signoff -m "refactor(paging): NodeFrameOps 新增 reclaim 方法"
```

---

### Task 3: PageTable 重构——CAS map + atomic unmap + Drop walk

**Files:**
- Modify: `crates/paging/src/table.rs`

这是改动量最大、风险最高的 task。完全重写 `table.rs`。

- [ ] **Step 1: 重写 PageTable 结构体和 create 方法**

删除 `NodeEntry`、`BTreeMap` import、`nodes`、`root_ref_count`。

```rust
//! 多级页表——walk / map / unmap 逻辑。

use crate::error::PagingError;
use crate::{
    ENTRIES_PER_TABLE, NodeFrame, NodeFrameOps, PageTableEntry, PteFlags, PteFlagsOps, PteOps,
    Table, vpn_index,
};
use address::{PhysAddr, VirtAddr};

const PT_LEVELS: usize = config::PT_LEVELS;

/// 多级页表。
///
/// 拥有根帧；中间帧的所有权编码在 PTE 中（分配后 `forget`），
/// `Drop` 时递归遍历整棵树回收。
///
/// 所有操作均为 `&self`，通过 `AtomicU64` CAS/swap 实现无锁并发。
///
/// 具体帧类型由 [`NodeFrame`] 类型别名决定（裸机：物理帧，测试：堆分配帧），
/// 无需泛型参数。
pub struct PageTable {
    root_paddr: PhysAddr,
    /// 持有根帧所有权，阻止帧被释放——字段本身不直接访问。
    #[expect(dead_code, reason = "仅用于持有所有权，通过 root_paddr 访问")]
    root: NodeFrame,
}

// SAFETY: PageTable 的所有 PTE 操作通过 AtomicU64 保证 SMP 安全；
// root 字段仅持有所有权（#[expect(dead_code)]），不存在并发数据访问。
unsafe impl Sync for PageTable {}
```

- [ ] **Step 2: 实现 create 和 root_paddr**

```rust
impl PageTable {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, PagingError> {
        let root = NodeFrame::alloc()?;
        let root_paddr = root.paddr();
        Ok(Self { root_paddr, root })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root_paddr
    }
}
```

- [ ] **Step 3: 实现 walk_readonly 和 get_mapping（Acquire load）**

```rust
impl PageTable {
    /// 只读遍历——从根向下查找叶 PTE，返回 PTE 及其所在层级。
    fn walk_readonly(&self, va: VirtAddr) -> Option<(PageTableEntry, usize)> {
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self.root 或 PTE 编码的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read_acquire(idx);
            if !pte.is_valid() {
                return None;
            }
            if pte.is_leaf(level) {
                return Some((pte, level));
            }
            paddr = pte.paddr();
        }

        // SAFETY: paddr 指向有效帧
        let table = unsafe { Table::from_paddr(paddr) };
        let idx = vpn_index(va, 0);
        let pte = table.read_acquire(idx);
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
}
```

- [ ] **Step 4: 实现 walk_to_leaf_location 和 atomic_clear_leaf**

```rust
impl PageTable {
    /// 定位叶 PTE 的位置——walk_readonly 的变体，返回 (Table, index, level)。
    fn walk_to_leaf_location(&self, va: VirtAddr) -> Option<(Table, usize, usize)> {
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read_acquire(idx);
            if !pte.is_valid() {
                return None;
            }
            if pte.is_leaf(level) {
                return Some((table, idx, level));
            }
            paddr = pte.paddr();
        }

        // SAFETY: paddr 指向有效帧
        let table = unsafe { Table::from_paddr(paddr) };
        let idx = vpn_index(va, 0);
        let pte = table.read_acquire(idx);
        if pte.is_valid() && pte.is_leaf(0) {
            Some((table, idx, 0))
        } else {
            None
        }
    }

    /// 原子清除叶 PTE——返回旧 PA 和 flags。
    ///
    /// 不维护 ref_count，不回收中间节点。
    /// 中间节点一旦安装永不删除，walk 路径无竞态。
    pub(crate) fn atomic_clear_leaf(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
        let (table, idx, _level) = self.walk_to_leaf_location(va)?;
        let old_raw = table.swap(idx, 0);
        let old_pte = PageTableEntry::from_raw(old_raw);
        old_pte
            .is_valid()
            .then(|| (old_pte.paddr(), old_pte.flags()))
    }
}
```

- [ ] **Step 5: 实现 walk_create_cas 和 map_page / map_at_level**

```rust
impl PageTable {
    /// CAS 遍历——按需分配中间节点，CAS 竞争安装。
    fn walk_create_cas(
        &self,
        va: VirtAddr,
        target_level: usize,
    ) -> Result<(Table, usize), PagingError> {
        let mut paddr = self.root_paddr;

        for level in (target_level + 1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read_acquire(idx);

            if !pte.is_valid() {
                let frame = NodeFrame::alloc()?;
                let frame_paddr = frame.paddr();
                let new_pte = PageTableEntry::new_intermediate(frame_paddr);

                match table.compare_exchange(idx, 0, new_pte.as_raw()) {
                    Ok(_) => {
                        // CAS 成功——所有权编码到 PTE，forget 阻止 drop
                        core::mem::forget(frame);
                        paddr = frame_paddr;
                    }
                    Err(actual) => {
                        // CAS 失败——其他核已安装，释放多余分配，使用赢家的节点
                        drop(frame);
                        let winner = PageTableEntry::from_raw(actual);
                        debug_assert!(
                            winner.is_valid(),
                            "CAS 失败但旧值无效——内核状态已损坏"
                        );
                        if winner.is_leaf(level) {
                            return Err(PagingError::HugePageConflict);
                        }
                        paddr = winner.paddr();
                    }
                }
            } else if pte.is_leaf(level) {
                return Err(PagingError::HugePageConflict);
            } else {
                paddr = pte.paddr();
            }
        }

        // SAFETY: paddr 指向有效帧
        let table = unsafe { Table::from_paddr(paddr) };
        let idx = vpn_index(va, target_level);
        Ok((table, idx))
    }

    /// 映射单个虚拟页到物理帧（Level 0，4KB）。
    ///
    /// 使用 CAS 安装叶 PTE——完全无锁。
    ///
    /// # Errors
    ///
    /// - 该 VA 已被映射时返回 `AlreadyMapped`。
    /// - walk 路径上遇到大页时返回 `HugePageConflict`。
    pub(crate) fn map_page(
        &self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PagingError> {
        self.map_at_level(va, pa, flags, 0)
    }

    /// 在指定层级映射虚拟地址到物理地址。
    ///
    /// 使用 CAS 安装叶 PTE——完全无锁。
    ///
    /// # Errors
    ///
    /// - 该 VA 已被映射时返回 `AlreadyMapped`。
    /// - walk 路径上遇到大页时返回 `HugePageConflict`。
    pub(crate) fn map_at_level(
        &self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
        level: usize,
    ) -> Result<(), PagingError> {
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

        let (table, idx) = self.walk_create_cas(va, level)?;
        let leaf_flags = flags.for_leaf_at_level(level);
        let leaf_pte = PageTableEntry::new(pa, leaf_flags);
        match table.compare_exchange(idx, 0, leaf_pte.as_raw()) {
            Ok(_) => Ok(()),
            Err(_) => Err(PagingError::AlreadyMapped),
        }
    }
}
```

- [ ] **Step 6: 实现 identity_map_range**

```rust
impl PageTable {
    /// 将 `[start, end)` 物理地址区间 identity-map（VA == PA）。
    ///
    /// 自动使用最大可用页大小（1GB / 2MB / 4KB）。
    /// 映射失败时直接 panic——内核启动阶段的 identity map 失败不可恢复。
    ///
    /// # Panics
    ///
    /// `start >= end` 或映射冲突时 panic。
    pub(crate) fn identity_map_range(&self, start: PhysAddr, end: PhysAddr, flags: PteFlags) {
        let mut addr = start.align_down();
        let end_aligned = end.align_up();

        assert!(
            addr.as_usize() < end_aligned.as_usize(),
            "identity_map_range: 无效地址范围 [{addr}, {end_aligned})"
        );

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

            self.map_at_level(va, addr, flags, selected_level)
                .expect("identity_map_range: 映射失败");
            addr += selected_size;
        }
    }
}
```

- [ ] **Step 7: 实现 PageTable::drop——递归 walk 回收中间节点**

```rust
impl PageTable {
    /// 递归回收中间节点帧——PageTable::drop 的核心。
    ///
    /// `level == 0` 时返回（L0 叶帧由 MappedPages/reclaim_exclusive_frame 管理）。
    /// 对每个非叶的有效中间 PTE：先递归子树，再回收该节点帧。
    fn reclaim_children(&self, paddr: PhysAddr, level: usize) {
        if level == 0 {
            return;
        }
        // SAFETY: paddr 指向有效帧（root 或由 CAS 安装的中间节点）
        let table = unsafe { Table::from_paddr(paddr) };
        for idx in 0..ENTRIES_PER_TABLE {
            let pte = table.read_acquire(idx);
            if pte.is_valid() && !pte.is_leaf(level) {
                let child = pte.paddr();
                self.reclaim_children(child, level - 1);
                // SAFETY: child 帧由 walk_create_cas 中 forget 的 NodeFrame 分配，
                // PageTable 独占所有权（Arc refcount == 0），无并发访问。
                unsafe { NodeFrame::reclaim(child) };
            }
        }
    }
}

impl Drop for PageTable {
    fn drop(&mut self) {
        // root 帧由 self.root 持有，Drop 自动释放。
        // 递归回收所有中间节点帧。
        self.reclaim_children(self.root_paddr, PT_LEVELS - 1);
    }
}
```

- [ ] **Step 8: 重写 table.rs 测试——删除所有 `&mut pt` 改为 `&pt`**

所有测试中 `let mut pt = PageTable::create()` 改为 `let pt = PageTable::create()`，
`pt.map_page(...)` / `pt.identity_map_range(...)` 不再需要 `&mut`。
删除所有 `unmap_page` / `unmap_at_level` 测试（这些方法已删除），
改为使用 `atomic_clear_leaf` 测试 unmap 功能。

测试列表保持与原始测试覆盖相同：

```rust
#[cfg(test)]
mod tests {
    use crate::error::PagingError;
    use crate::*;
    use address::{PhysAddr, VirtAddr};

    type PageTable = crate::table::PageTable;

    #[test]
    fn root_paddr_is_valid() {
        let pt = PageTable::create().expect("创建测试页表失败");
        assert_ne!(pt.root_paddr().as_usize(), 0);
    }

    #[test]
    fn map_and_get_mapping() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let va = VirtAddr::new(0x1000);
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PteFlags::kernel_rw();
        pt.map_page(va, pa, flags).expect("map_page 应成功");
        let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能找到映射");
        assert_eq!(mapped_pa, pa);
        assert_eq!(mapped_flags, flags);
    }

    #[test]
    fn map_different_pages() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let va1 = VirtAddr::new(0x0000_1000);
        let va2 = VirtAddr::new(0x0000_2000);
        let pa1 = PhysAddr::new(0x8020_0000);
        let pa2 = PhysAddr::new(0x8020_1000);
        pt.map_page(va1, pa1, PteFlags::kernel_rw()).expect("map va1");
        pt.map_page(va2, pa2, PteFlags::kernel_rx()).expect("map va2");
        let (got_pa1, got_flags1) = pt.get_mapping(va1).expect("va1 应已映射");
        let (got_pa2, got_flags2) = pt.get_mapping(va2).expect("va2 应已映射");
        assert_eq!(got_pa1, pa1);
        assert_eq!(got_pa2, pa2);
        assert_eq!(got_flags1, PteFlags::kernel_rw());
        assert_eq!(got_flags2, PteFlags::kernel_rx());
    }

    #[test]
    fn double_map_fails() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let va = VirtAddr::new(0x1000);
        let pa = PhysAddr::new(0x8020_0000);
        pt.map_page(va, pa, PteFlags::kernel_rw()).expect("首次 map 应成功");
        let err = pt.map_page(va, pa, PteFlags::kernel_rw()).expect_err("重复 map 应失败");
        assert_eq!(err, PagingError::AlreadyMapped);
    }

    #[test]
    fn atomic_clear_leaf_returns_old() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let va = VirtAddr::new(0x1000);
        let pa = PhysAddr::new(0x8020_0000);
        pt.map_page(va, pa, PteFlags::kernel_rw()).expect("map 应成功");
        let (old_pa, _) = pt.atomic_clear_leaf(va).expect("atomic_clear_leaf 应成功");
        assert_eq!(old_pa, pa);
        assert!(pt.get_mapping(va).is_none());
    }

    #[test]
    fn atomic_clear_leaf_unmapped_returns_none() {
        let pt = PageTable::create().expect("创建测试页表失败");
        assert!(pt.atomic_clear_leaf(VirtAddr::new(0x1000)).is_none());
    }

    #[test]
    fn remap_after_atomic_clear() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let va = VirtAddr::new(0x1000);
        let pa1 = PhysAddr::new(0x8020_0000);
        let pa2 = PhysAddr::new(0x8030_0000);
        pt.map_page(va, pa1, PteFlags::kernel_rw()).expect("首次 map");
        pt.atomic_clear_leaf(va).expect("clear");
        pt.map_page(va, pa2, PteFlags::kernel_rx()).expect("重映射应成功");
        let (got_pa, got_flags) = pt.get_mapping(va).expect("应找到新映射");
        assert_eq!(got_pa, pa2);
        assert_eq!(got_flags, PteFlags::kernel_rx());
    }

    #[test]
    fn map_pages_in_different_vpn_ranges() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let va_low = VirtAddr::new(0x0000_1000);
        let va_high = VirtAddr::new(0x4000_0000);
        let pa1 = PhysAddr::new(0x8020_0000);
        let pa2 = PhysAddr::new(0x8020_1000);
        pt.map_page(va_low, pa1, PteFlags::kernel_rw()).expect("map low");
        pt.map_page(va_high, pa2, PteFlags::kernel_rw()).expect("map high");
        let (got1, _) = pt.get_mapping(va_low).expect("low 应已映射");
        let (got2, _) = pt.get_mapping(va_high).expect("high 应已映射");
        assert_eq!(got1, pa1);
        assert_eq!(got2, pa2);
    }

    #[test]
    fn get_mapping_on_empty_table() {
        let pt = PageTable::create().expect("创建测试页表失败");
        assert!(pt.get_mapping(VirtAddr::new(0x1000)).is_none());
        assert!(pt.get_mapping(VirtAddr::new(0)).is_none());
    }

    #[test]
    fn identity_map_range_multi_page() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let start = PhysAddr::new(0x10_0000);
        let end = PhysAddr::new(0x10_3000);
        pt.identity_map_range(start, end, PteFlags::kernel_rw());
        for i in 0..3 {
            let va = VirtAddr::new(0x10_0000 + i * config::PAGE_SIZE);
            let (pa, _) = pt.get_mapping(va).expect("应能查到映射");
            assert_eq!(pa, PhysAddr::new(0x10_0000 + i * config::PAGE_SIZE));
        }
    }

    #[test]
    fn map_at_level1_huge_page() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let huge_size = page_size_at_level(1);
        let va = VirtAddr::new(huge_size);
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PteFlags::kernel_rw();
        pt.map_at_level(va, pa, flags, 1).expect("大页映射应成功");
        let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能查询到大页映射");
        assert_eq!(mapped_pa, pa);
        assert_eq!(mapped_flags, flags.for_leaf_at_level(1));
    }

    #[test]
    fn get_mapping_within_huge_page() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let huge_size = page_size_at_level(1);
        let va_base = VirtAddr::new(huge_size);
        let pa = PhysAddr::new(0x8020_0000);
        pt.map_at_level(va_base, pa, PteFlags::kernel_rw(), 1).expect("大页映射应成功");
        let (got_pa_base, _) = pt.get_mapping(va_base).expect("大页基地址应命中映射");
        assert_eq!(got_pa_base, pa);
        let va_offset = VirtAddr::new(huge_size + 0x1000);
        let (got_pa, _) = pt.get_mapping(va_offset).expect("大页内偏移地址应命中映射");
        assert_eq!(got_pa, pa + 0x1000);
    }

    #[test]
    fn map_page_under_huge_page_fails() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let huge_size = page_size_at_level(1);
        let va = VirtAddr::new(huge_size);
        let pa = PhysAddr::new(0x8020_0000);
        pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1).expect("大页映射应成功");
        let sub_va = VirtAddr::new(huge_size + 0x1000);
        let err = pt.map_page(sub_va, PhysAddr::new(0x9000_0000), PteFlags::kernel_rw())
            .expect_err("大页范围内的子映射应失败");
        assert_eq!(err, PagingError::HugePageConflict);
    }

    #[test]
    fn double_map_at_level_fails() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let huge_size = page_size_at_level(1);
        let va = VirtAddr::new(huge_size);
        let pa = PhysAddr::new(0x8020_0000);
        pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1).expect("首次大页映射应成功");
        let err = pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
            .expect_err("重复大页映射应失败");
        assert_eq!(err, PagingError::AlreadyMapped);
    }

    #[test]
    #[should_panic(expected = "无效地址范围")]
    fn identity_map_range_equal_range_panics() {
        let pt = PageTable::create().expect("创建测试页表失败");
        pt.identity_map_range(PhysAddr::new(0x10_0000), PhysAddr::new(0x10_0000), PteFlags::kernel_rw());
    }

    #[test]
    #[should_panic(expected = "无效地址范围")]
    fn identity_map_range_reversed_range_panics() {
        let pt = PageTable::create().expect("创建测试页表失败");
        pt.identity_map_range(PhysAddr::new(0x20_0000), PhysAddr::new(0x10_0000), PteFlags::kernel_rw());
    }

    #[test]
    fn identity_map_range_auto_huge_page() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let huge_size = page_size_at_level(1);
        let start = PhysAddr::new(huge_size);
        let end = PhysAddr::new(huge_size * 2);
        pt.identity_map_range(start, end, PteFlags::kernel_rw());
        let (pa, flags) = pt.get_mapping(VirtAddr::new(huge_size)).expect("大页基址应已映射");
        assert_eq!(pa, start);
        assert_eq!(flags, PteFlags::kernel_rw().for_leaf_at_level(1));
        let (pa_offset, _) = pt.get_mapping(VirtAddr::new(huge_size + 0x1000)).expect("大页内偏移应命中");
        assert_eq!(pa_offset, PhysAddr::new(huge_size + 0x1000));
    }

    #[test]
    #[should_panic(expected = "映射失败")]
    fn identity_map_range_conflict_panics() {
        let pt = PageTable::create().expect("创建测试页表失败");
        let conflict_va = VirtAddr::new(0x10_2000);
        let conflict_pa = PhysAddr::new(0x10_2000);
        pt.map_page(conflict_va, conflict_pa, PteFlags::kernel_rw()).expect("占位映射应成功");
        pt.identity_map_range(PhysAddr::new(0x10_0000), PhysAddr::new(0x10_3000), PteFlags::kernel_rw());
    }

    /// PageTable drop 后中间节点帧应被回收（不 panic，不泄漏）。
    #[test]
    fn drop_reclaims_intermediate_nodes() {
        let pt = PageTable::create().expect("创建页表");
        pt.map_page(VirtAddr::new(0x1000), PhysAddr::new(0x8020_0000), PteFlags::kernel_rw())
            .expect("map");
        pt.map_page(VirtAddr::new(0x4000_0000), PhysAddr::new(0x8020_1000), PteFlags::kernel_rw())
            .expect("map high");
        drop(pt);
        // 不 panic 即通过——reclaim_children 成功遍历并回收了所有中间节点
    }
}
```

- [ ] **Step 9: 运行 table.rs 测试**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging -- table 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 10: Commit**

```bash
git add crates/paging/src/table.rs
git commit --signoff -m "refactor(paging): PageTable 完全无锁——CAS map + atomic unmap + Drop walk"
```

---

### Task 4: MappedPagesInner 精简 + unmap_and_reclaim 无锁化

**Files:**
- Modify: `crates/paging/src/mapping.rs`

- [ ] **Step 1: 精简 MappedPagesInner——删除 flags/exclusive，改 Arc 类型**

将 `MappedPagesInner` 从 5 个字段缩减到 3 个，所有方法适配无锁 PageTable。

`mapping.rs` 顶部 import 修改：

```rust
use alloc::sync::Arc;
use core::ops::{Deref, DerefMut};

use address::{FrameRange, PhysAddr, PhysPageNum, VirtAddr};
use config::PAGE_SIZE;
use frame_allocator::{AllocatedFrames, UnmappedFrames};

use crate::error::PagingError;
use crate::{PageTable, PteFlags, PteFlagsOps};
```

删除 `use sync_crate::SpinLock;`。

MappedPagesInner 新定义：

```rust
/// 映射的内部共享数据——`MappedPages` 和 `PermanentMapping` 通过 `Deref` 共享。
///
/// 不可直接构造——只能通过 `MappedPages` 或 `PermanentMapping` 的工厂方法创建。
pub struct MappedPagesInner {
    /// 映射起始虚拟地址
    vaddr: VirtAddr,
    /// 映射的页数
    page_count: usize,
    /// 所属页表的 `Arc` 引用——Drop 时通过此引用 atomic unmap。
    page_table: Arc<PageTable>,
}
```

共享方法：

```rust
impl MappedPagesInner {
    /// 返回映射起始虚拟地址。
    #[must_use]
    pub fn vaddr(&self) -> VirtAddr {
        self.vaddr
    }

    /// 返回映射总大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.page_count * PAGE_SIZE
    }

    /// 返回构造时的请求权限（从 PTE 读取，去掉 EXCLUSIVE）。
    ///
    /// **性能注意**：每次调用触发一次页表 walk（O(PT_LEVELS) Acquire load）。
    /// 当前调用者为 VMA 构造和 Debug 格式化，均为低频路径。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.page_table
            .get_mapping(self.vaddr)
            .map(|(_, flags)| flags.without_exclusive())
            .expect("MappedPagesInner::flags: 映射不存在")
    }

    /// 读取指定偏移所在页的实际 PTE 标志。
    ///
    /// 无锁——直接通过 PageTable::get_mapping 原子读取。
    ///
    /// # Panics
    ///
    /// 映射不存在时 panic（不应在正常使用中发生）。
    #[must_use]
    pub fn pte_flags(&self, offset: usize) -> PteFlags {
        let page_va = (self.vaddr + offset).align_down();
        self.page_table
            .get_mapping(page_va)
            .expect("MappedPagesInner::pte_flags: 映射不存在")
            .1
    }

    // as_type 和 as_type_mut 保持不变（它们调用 pte_flags，已无锁）
```

- [ ] **Step 2: 修改 unmap_and_reclaim——使用 atomic_clear_leaf**

```rust
    /// 从所属页表中 unmap 所有页，EXCLUSIVE 帧自动回收。
    ///
    /// 完全无锁——使用 PageTable::atomic_clear_leaf 原子清除叶 PTE。
    /// 使用栈上固定大小数组分块处理，避免堆分配。
    ///
    /// 每个 chunk 内的操作顺序（SMP 安全）：
    /// 1. 原子清除 PTE 并收集 EXCLUSIVE 帧地址（无锁）
    /// 2. TLB flush（确保所有核心的 stale TLB 失效）
    /// 3. 回收物理帧（此时没有核心持有指向这些帧的 TLB 条目）
    fn unmap_and_reclaim(&self) {
        let mut offset = 0;
        while offset < self.page_count {
            let n = (self.page_count - offset).min(UNMAP_CHUNK);
            let mut exclusive_pas: heapless::Vec<PhysAddr, UNMAP_CHUNK> = heapless::Vec::new();

            for i in 0..n {
                let va = self.vaddr + (offset + i) * PAGE_SIZE;
                match self.page_table.atomic_clear_leaf(va) {
                    Some((pa, flags)) => {
                        if flags.is_exclusive() {
                            exclusive_pas
                                .push(pa)
                                .expect("exclusive 帧数不超过 UNMAP_CHUNK");
                        }
                    }
                    None => {
                        panic!(
                            "MappedPages::unmap_and_reclaim: unmap {va} 失败——\
                             MappedPages 保证映射存在，此错误说明内核状态已损坏"
                        );
                    }
                }
            }

            // TLB flush——必须在帧回收之前完成
            {
                let flush_va = self.vaddr + offset * PAGE_SIZE;
                let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);
            }

            // 所有核心的 TLB 已刷新，安全回收帧
            for pa in &exclusive_pas {
                reclaim_exclusive_frame(*pa);
            }

            offset += n;
        }
    }
}
```

- [ ] **Step 3: 修改工厂方法签名——Arc\<SpinLock\<PageTable\>\> → Arc\<PageTable\>**

`map_identity`：

```rust
    pub fn map_identity(
        pt_ref: Arc<PageTable>,
        pa_start: PhysAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Result<Self, PagingError> {
        assert!(page_count > 0, "MappedPages::map_identity: page_count 不能为 0");
        let va_start = VirtAddr::new(pa_start.as_usize());
        let pa_end = pa_start + page_count * PAGE_SIZE;
        pt_ref.identity_map_range(pa_start, pa_end, flags);
        Ok(Self(MappedPagesInner {
            vaddr: va_start,
            page_count,
            page_table: pt_ref,
        }))
    }
```

`wrap_existing`：

```rust
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn wrap_existing(
        pt_ref: Arc<PageTable>,
        vaddr: VirtAddr,
        page_count: usize,
        _flags: PteFlags,
    ) -> Self {
        debug_assert!(page_count > 0);
        Self(MappedPagesInner {
            vaddr,
            page_count,
            page_table: pt_ref,
        })
    }
```

`map_alloc`：

```rust
    pub fn map_alloc(
        pt_ref: Arc<PageTable>,
        va_start: VirtAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Self {
        assert!(page_count > 0, "MappedPages::map_alloc: page_count 不能为 0");
        let exclusive_flags = flags.with_exclusive();
        let mut offset = 0;

        while offset < page_count {
            let n = (page_count - offset).min(MAP_CHUNK);

            // 1. 分配帧到栈缓冲（不用堆分配）
            let mut frames: heapless::Vec<AllocatedFrames, MAP_CHUNK> = heapless::Vec::new();
            for _ in 0..n {
                let frame =
                    AllocatedFrames::alloc_one().expect("map_alloc: 帧分配失败（物理内存耗尽）");
                frames
                    .push(frame)
                    .unwrap_or_else(|_| panic!("map_alloc: 帧数不超过 MAP_CHUNK"));
            }

            // 2. CAS 映射（无锁）
            for (i, frame) in frames.into_iter().enumerate() {
                let pa = frame.start_paddr();
                let va = va_start + (offset + i) * PAGE_SIZE;
                pt_ref
                    .map_page(va, pa, exclusive_flags)
                    .expect("map_alloc: map_page 失败（VA 冲突说明调用方 VMA 管理有 bug）");
                // 帧所有权转移到 PTE：forget 阻止 drop 回收
                let mapped = frame.into_mapped();
                core::mem::forget(mapped);
            }

            offset += n;
        }

        Self(MappedPagesInner {
            vaddr: va_start,
            page_count,
            page_table: pt_ref,
        })
    }
```

- [ ] **Step 4: 修改 Debug 实现——从 PTE 读取 exclusive 状态**

```rust
impl core::fmt::Debug for MappedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let kind = self
            .0
            .page_table
            .get_mapping(self.0.vaddr)
            .map(|(_, flags)| {
                if flags.is_exclusive() {
                    "exclusive"
                } else {
                    "borrowed"
                }
            })
            .unwrap_or("unmapped");
        write!(
            f,
            "MappedPages({}, {} pages, {})",
            self.0.vaddr, self.0.page_count, kind
        )
    }
}

impl core::fmt::Debug for PermanentMapping {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "PermanentMapping({}, {} pages)",
            self.0.vaddr, self.0.page_count
        )
    }
}
```

- [ ] **Step 5: 修改测试——删除所有 lock() 调用**

所有测试中的 `pt_ref.lock()` 改为直接调用 `pt_ref.xxx()`。`wrap_existing` 的 `flags` 参数仍传入但标记为 `_flags`（仅用于 map_page 调用的测试场景）。

关键变更模式：

```rust
// 原来：
let guard = pt_ref.lock();
let (got_pa, got_flags) = guard.get_mapping(va).expect("...");

// 改为：
let (got_pa, got_flags) = pt_ref.get_mapping(va).expect("...");
```

```rust
// 原来：
let mut guard = pt_ref.lock();
guard.map_page(va, pa, flags).expect("...");

// 改为：
pt_ref.map_page(va, pa, flags).expect("...");
```

对于 `drop_identity_unmaps_pte` 和 `drop_alloc_unmaps_and_reclaims` 测试，drop 后直接调用 `pt_ref.get_mapping(va)` 验证（不需要 lock）。

- [ ] **Step 6: 运行 paging crate 全部测试**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 7: Commit**

```bash
git add crates/paging/src/mapping.rs
git commit --signoff -m "refactor(paging): MappedPagesInner 无锁化——删除 flags/exclusive，atomic unmap"
```

---

### Task 5: paging crate re-export + test_pt 适配

**Files:**
- Modify: `crates/paging/src/lib.rs:38-43`

- [ ] **Step 1: 修改 test_pt 返回类型**

```rust
/// 创建测试用 `Arc<PageTable>`。
#[cfg(any(test, feature = "test-support"))]
pub fn test_pt() -> alloc::sync::Arc<PageTable> {
    let pt = PageTable::create().expect("创建页表");
    alloc::sync::Arc::new(pt)
}
```

- [ ] **Step 2: 运行 paging crate 测试**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 3: Commit**

```bash
git add crates/paging/src/lib.rs
git commit --signoff -m "refactor(paging): test_pt 返回 Arc<PageTable>"
```

---

### Task 6: MmioRegion 适配

**Files:**
- Modify: `crates/paging/src/mmio.rs`

- [ ] **Step 1: 修改 map_to 签名和 import**

删除 `use sync_crate::SpinLock;`，修改 `map_to` 签名：

```rust
use alloc::sync::Arc;

use crate::error::PagingError;
use crate::mapping::{MappedPages, PermanentMapping, check_bounds_and_align};
use crate::{PageTable, PteFlags, PteFlagsOps};
use address::PhysAddr;
use config::PAGE_SIZE;

// ...

    pub fn map_to(
        pt_ref: Arc<PageTable>,
        paddr: PhysAddr,
        size: usize,
    ) -> Result<Self, PagingError> {
        // 函数体不变
```

- [ ] **Step 2: 运行 paging crate 测试**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 3: Commit**

```bash
git add crates/paging/src/mmio.rs
git commit --signoff -m "refactor(paging): MmioRegion::map_to 签名适配 Arc<PageTable>"
```

---

### Task 7: memory crate 适配——vma + globals + init + lib

**Files:**
- Modify: `crates/memory/src/vma.rs:9,16,135-158,204,236,330-331,340-341,385`
- Modify: `crates/memory/src/globals.rs:6-8,33-55`
- Modify: `crates/memory/src/init.rs:88-91`
- Modify: `crates/memory/src/lib.rs:78`

- [ ] **Step 1: 修改 vma.rs——AddressSpace 字段和方法签名**

删除 `use sync_crate::SpinLock;`，修改 `AddressSpace`：

```rust
use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use crate::MappedPages;
use crate::PermanentMapping;
use crate::error::MemoryError;
use crate::node_frame::PageTable;
use address::{AddrRange, VirtAddr};
use config::PAGE_SIZE;
use paging::{PteFlags, PteFlagsOps};
```

```rust
/// # 所有权模型
///
/// `AddressSpace` 通过 `Arc<PageTable>` 共享页表引用。
/// PageTable 完全无锁——map 使用 CAS，unmap 使用 atomic swap。
/// 内核地址空间共享全局内核页表；用户进程各自持有独立页表的 `Arc`。
/// 进程退出后 `Arc` 引用计数归零，页表自动释放。
pub struct AddressSpace {
    /// 所属页表的 `Arc` 引用
    page_table: Arc<PageTable>,
    /// VMA 集合——以起始虚拟地址为键，保证有序且不重叠
    areas: BTreeMap<VirtAddr, Vma>,
}

impl AddressSpace {
    /// 创建空地址空间。
    pub fn new(page_table: Arc<PageTable>) -> Self {
        Self {
            page_table,
            areas: BTreeMap::new(),
        }
    }

    /// 返回页表的 `Arc` 引用。
    #[must_use]
    pub fn page_table(&self) -> &Arc<PageTable> {
        &self.page_table
    }
```

`mmap_anonymous`、`mmap_identity`、`handle_page_fault`、`mmap_identity_range` 中的 `self.page_table.clone()` 类型自动适配——`Arc<PageTable>` 的 `clone()` 与 `Arc<SpinLock<PageTable>>` 的 `clone()` 签名相同。无需修改函数体。

- [ ] **Step 2: 修改 globals.rs——KERNEL_PAGE_TABLE 类型**

```rust
#[cfg(any(test, target_os = "none"))]
use alloc::sync::Arc;

#[cfg(any(test, target_os = "none"))]
use crate::node_frame::PageTable;
#[cfg(any(test, target_os = "none"))]
use crate::vma::AddressSpace;

// ...（MemoryInfo 和 MEMORY_INFO 不变）

/// 全局内核页表（`Arc` 共享引用——内核与所有内核线程共享同一页表）。
#[cfg(any(test, target_os = "none"))]
static KERNEL_PAGE_TABLE: spin::Once<Arc<PageTable>> = spin::Once::new();

/// 全局内核地址空间。
#[cfg(any(test, target_os = "none"))]
static KERNEL_ADDRESS_SPACE: spin::Once<sync_crate::SpinLock<AddressSpace>> = spin::Once::new();

/// 将构建完成的内核页表存入全局 `KERNEL_PAGE_TABLE`。
#[cfg(any(test, target_os = "none"))]
pub fn store_kernel_page_table(pt: PageTable) {
    KERNEL_PAGE_TABLE.call_once(|| Arc::new(pt));
}

/// 获取全局内核页表的 `Arc` 引用；初始化前返回 `None`。
#[cfg(any(test, target_os = "none"))]
pub fn kernel_page_table() -> Option<Arc<PageTable>> {
    KERNEL_PAGE_TABLE.get().cloned()
}
```

删除 `use sync_crate::SpinLock;`（仅 `KERNEL_ADDRESS_SPACE` 仍使用，保留该 import 但改为只在 `KERNEL_ADDRESS_SPACE` 行直接使用完整路径，或保留 import）。

注意：`KERNEL_ADDRESS_SPACE` 仍使用 `SpinLock<AddressSpace>`——这是对地址空间 VMA 操作的锁，与页表锁无关，保留。

- [ ] **Step 3: 修改 init.rs——init_smp 删除 lock**

```rust
/// 从核内存初始化——复用主核页表并激活分页。
pub fn init_smp(activate: impl FnOnce(&PageTable)) {
    let kpt = crate::globals::kernel_page_table().expect("KERNEL_PAGE_TABLE not initialized");
    activate(&*kpt);
    log::info!(
        "MemoryInitSMP: paging enabled on core {}",
        per_cpu::current_core_id()
    );
}
```

- [ ] **Step 4: 修改 lib.rs——map_mmio 中 kpt 类型自动适配**

`map_mmio` 中的 `let kpt = kernel_page_table()` 返回 `Arc<PageTable>`，传给 `MmioRegion::map_to(kpt, ...)` 已适配。无需修改函数体。

检查是否有 `SpinLock` 相关 import 需要清理。

- [ ] **Step 5: 运行 memory crate 测试**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p memory 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 6: Commit**

```bash
git add crates/memory/src/vma.rs crates/memory/src/globals.rs crates/memory/src/init.rs crates/memory/src/lib.rs
git commit --signoff -m "refactor(memory): 适配 Arc<PageTable> 无锁接口"
```

---

### Task 8: 内核入口适配——main.rs + boot.rs

**Files:**
- Modify: `src/main.rs:58-61`
- Modify: `src/boot.rs:33-36`

- [ ] **Step 1: 修改 main.rs——删除 lock()**

```rust
    // 原来：
    // {
    //     let pt = kernel_as.page_table().lock();
    //     unsafe { Arch::activate_page_table(&pt) };
    // }

    // 改为：
    unsafe { Arch::activate_page_table(&**kernel_as.page_table()) };
```

`kernel_as.page_table()` 返回 `&Arc<PageTable>`，`&**` 解引用为 `&PageTable`。

- [ ] **Step 2: 修改 boot.rs——删除 lock()**

```rust
    // 原来：
    // {
    //     let pt = kernel_as.page_table().lock();
    //     unsafe { Arch::activate_page_table(&pt) };
    // }

    // 改为：
    unsafe { Arch::activate_page_table(&**kernel_as.page_table()) };
```

- [ ] **Step 3: Commit**

```bash
git add src/main.rs src/boot.rs
git commit --signoff -m "refactor: 内核入口适配 Arc<PageTable> 无锁接口"
```

---

### Task 9: 最终验证

- [ ] **Step 1: 全量单元测试**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 2: Clippy**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo clippy -- -D warnings 2>&1 | tail -20`
Expected: 无 warning

- [ ] **Step 3: 格式检查**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo fmt --check 2>&1`
Expected: 无格式问题

- [ ] **Step 4: 裸机交叉编译验证**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo xtask build --arch riscv64 2>&1 | tail -20`
Expected: 编译成功

- [ ] **Step 5: 确认无外层 SpinLock 残留**

Run: `grep -rn "SpinLock<PageTable>" crates/ src/ tests/ --include="*.rs"`
Expected: 无输出（所有 `SpinLock<PageTable>` 已消除）

- [ ] **Step 6: 确认无 lock() 残留（页表相关）**

Run: `grep -rn "page_table.*\.lock()\|pt_ref\.lock()\|kpt\.lock()" crates/ src/ tests/ --include="*.rs"`
Expected: 无输出
