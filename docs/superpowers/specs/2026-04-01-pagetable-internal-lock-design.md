# PageTable 完全无锁重构设计

## 问题

`PageTable` 被 `Arc<SpinLock<PageTable>>` 包裹，所有操作（map/unmap/query/Drop）争同一把锁。
导致两个问题：

1. **Drop 死锁**：`MappedPages::Drop` 调用 `unmap_and_reclaim`，获取 `page_table.lock()`。
   若调用方已持有同一锁，`SpinLock` 不可重入，死锁。Rust 的隐式 drop 让这个 bug 比 C 更容易触发。
2. **SMP 并行性差**：多核并发 map/unmap 不同 VA 区域被串行化。

详细分析见 `docs/rust-rewrite/issue-pagetable-lock-granularity.md`。

## 设计决策

经对比 Linux（分层锁 + CAS）、Theseus（无锁 + `&mut` 所有权）、Redox（单 RwLock），选择：

| 决策 | 选择 | 理由 |
|------|------|------|
| 锁 | **完全消除** | map 用 CAS 竞争安装中间节点，unmap 用 atomic swap |
| 中间节点跟踪 | **不跟踪**——所有权编码在 PTE 中 | 与 EXCLUSIVE 叶帧相同的模式：分配 → forget → PTE 记录 PA → Drop 时 walk 回收 |
| 中间节点回收 | 不回收（PageTable::drop 递归 walk 整棵树释放） | Theseus/Redox 验证可行 |
| Memory ordering | Acquire load / AcqRel CAS / AcqRel swap | 无锁需显式 ordering 保证跨核可见性 |
| `MappedPagesInner::flags` | **删除**——从 PTE 读取后去掉 EXCLUSIVE | 消除缓存副本，PTE 是唯一 source of truth |
| `MappedPagesInner::exclusive` | **删除**——从 PTE EXCLUSIVE 位判断 | 同上 |

## 架构

### PageTable 结构

```rust
pub struct PageTable {
    root_paddr: PhysAddr,
    root: NodeFrame,
    // 没有 SpinLock，没有 Vec，没有 BTreeMap
}

// SAFETY: PageTable 的所有 PTE 操作通过 AtomicU64 保证 SMP 安全；
// root 字段仅持有所有权（#[expect(dead_code)]），不存在并发数据访问。
unsafe impl Sync for PageTable {}
```

去掉外层 `SpinLock` 后，`Arc<PageTable>` 要求 `PageTable: Sync`。
`NodeFrame`（`HeapNodeFrame` 含 `*mut u8`）不自动 `Sync`，需手动 impl。

`create()` 简化为：

```rust
pub fn create() -> Result<Self, PagingError> {
    let root = NodeFrame::alloc()?;
    let root_paddr = root.paddr();
    Ok(Self { root_paddr, root })
}
```

删除 `NodeEntry`、`ref_count`、`inc_ref`/`dec_ref`/`ref_count_mut`、所有 `unmap_*` 方法。

### MappedPagesInner 结构

```rust
struct MappedPagesInner {
    vaddr: VirtAddr,
    page_count: usize,
    page_table: Arc<PageTable>,
    // 没有 flags，没有 exclusive
}
```

`flags()` 从 PTE 读取：`self.page_table.get_mapping(self.vaddr).flags().without_exclusive()`。
**性能 trade-off**：从 O(1) 字段读退化为 O(PT_LEVELS) 页表 walk（3-4 次 Acquire load）。
当前调用者仅有 VMA 构造（`mapping.flags()`，低频）和 Debug 格式化，可接受。

### 方法分类

所有方法均为 `&self`，全部无锁：

```
PageTable
├── root_paddr()              纯读
├── get_mapping(va)           walk (Acquire load) → 读叶 PTE
├── atomic_clear_leaf(va)     walk (Acquire load) → swap(0, AcqRel)
├── map_page(va, pa, flags)   walk_create_cas → CAS 安装中间节点 → CAS 安装叶 PTE
├── map_at_level(...)         同上，支持大页
└── identity_map_range(...)   循环调 map_at_level
```

### Table 原子操作

```rust
impl Table {
    /// Acquire load——无锁 walk 路径。
    fn read_acquire(&self, index: usize) -> PageTableEntry;

    /// AcqRel swap——无锁 unmap（原子清零叶 PTE）。
    fn swap(&self, index: usize, val: u64) -> u64;

    /// AcqRel CAS——无锁 map（安装中间节点或叶 PTE）。
    /// 返回 Result：Ok(old) CAS 成功，Err(actual) CAS 失败。
    /// 与 std::sync::atomic::AtomicU64::compare_exchange 语义一致。
    fn compare_exchange(&self, index: usize, expected: u64, new: u64) -> Result<u64, u64>;
}
```

删除 `read`（Relaxed load）和 `write`（Relaxed store）——无锁设计下所有操作都需要显式 ordering，
不再有"锁内路径可以 Relaxed"的场景。

### walk_create_cas——CAS 安装中间节点

```rust
fn walk_create_cas(&self, va: VirtAddr, target_level: usize)
    -> Result<(Table, usize), PagingError>
{
    let mut paddr = self.root_paddr;

    for level in (target_level + 1..PT_LEVELS).rev() {
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
                    debug_assert!(winner.is_valid(), "CAS 失败但旧值无效——内核状态已损坏");
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

    let table = unsafe { Table::from_paddr(paddr) };
    let idx = vpn_index(va, target_level);
    Ok((table, idx))
}
```

CAS 失败只在两个核同时 map 同一 VA 区域的第一个页时发生（竞争创建同一中间节点），
概率极低。失败方的帧 `drop` 立即归还分配器，无泄漏。

### map_page——CAS 安装叶 PTE

```rust
pub fn map_page(&self, va: VirtAddr, pa: PhysAddr, flags: PteFlags)
    -> Result<(), PagingError>
{
    let (table, idx) = self.walk_create_cas(va, 0)?;
    let leaf_pte = PageTableEntry::new(pa, flags.for_leaf_at_level(0));
    match table.compare_exchange(idx, 0, leaf_pte.as_raw()) {
        Ok(_) => Ok(()),
        Err(_) => Err(PagingError::AlreadyMapped),
    }
}
```

### atomic_clear_leaf——原子清除叶 PTE

```rust
pub fn atomic_clear_leaf(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
    let (table, idx, _level) = self.walk_to_leaf_location(va)?;
    let old_raw = table.swap(idx, 0);
    let old_pte = PageTableEntry::from_raw(old_raw);
    old_pte.is_valid().then(|| (old_pte.paddr(), old_pte.flags()))
}
```

### walk_to_leaf_location——定位叶 PTE 位置

`walk_readonly` 的变体，返回位置而非值，供 `atomic_clear_leaf` 使用：

```rust
fn walk_to_leaf_location(&self, va: VirtAddr) -> Option<(Table, usize, usize)> {
    let mut paddr = self.root_paddr;

    for level in (1..PT_LEVELS).rev() {
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

    let table = unsafe { Table::from_paddr(paddr) };
    let idx = vpn_index(va, 0);
    let pte = table.read_acquire(idx);
    if pte.is_valid() && pte.is_leaf(0) {
        Some((table, idx, 0))
    } else {
        None
    }
}
```

`walk_readonly` 同样从 `Table::read`（Relaxed）改为 `Table::read_acquire`（Acquire），
保持与无锁路径一致的 memory ordering。`get_mapping` 委托 `walk_readonly`，无需额外修改。

### PageTable::drop——递归 walk 回收中间节点

```rust
impl Drop for PageTable {
    fn drop(&mut self) {
        // root 帧由 self.root 持有，不需要手动回收
        // 递归回收所有中间节点帧
        self.reclaim_children(self.root_paddr, PT_LEVELS - 1);
    }
}

fn reclaim_children(&self, paddr: PhysAddr, level: usize) {
    if level == 0 {
        return; // L0 叶节点帧由 MappedPages/reclaim_exclusive_frame 管理
    }
    let table = unsafe { Table::from_paddr(paddr) };
    for idx in 0..ENTRIES_PER_TABLE {
        let pte = table.read_acquire(idx);
        if pte.is_valid() && !pte.is_leaf(level) {
            let child = pte.paddr();
            self.reclaim_children(child, level - 1);
            // SAFETY: child 帧由 walk_create_cas 中 forget 的 NodeFrame 分配，
            // PageTable 独占所有权，此时无并发访问。
            unsafe { NodeFrame::reclaim(child) };
        }
    }
}
```

### NodeFrameOps trait 扩展

```rust
pub trait NodeFrameOps: Send + Sized {
    fn alloc() -> Result<Self, error::PagingError>;
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

裸机实现：通过 `UnmappedFrames::from_range` 归还帧分配器（与 `reclaim_exclusive_frame` 相同模式）。
测试实现：从 paddr 重建指针并 `dealloc`——layout 硬编码为 `(PAGE_SIZE, PAGE_SIZE)`，
与 `HeapNodeFrame::alloc()` 中的 `Layout::from_size_align(PAGE_SIZE, PAGE_SIZE)` 必须一致。
这是 `forget` + `reclaim` 模式的固有约束：`alloc` 和 `reclaim` 的 layout 必须匹配。

### unmap_and_reclaim 改造

```rust
fn unmap_and_reclaim(&self) {
    let mut offset = 0;
    while offset < self.page_count {
        let n = (self.page_count - offset).min(UNMAP_CHUNK);
        let mut exclusive_pas: heapless::Vec<PhysAddr, UNMAP_CHUNK> = heapless::Vec::new();

        for i in 0..n {
            let va = self.vaddr + (offset + i) * PAGE_SIZE;
            if let Some((pa, flags)) = self.page_table.atomic_clear_leaf(va) {
                if flags.is_exclusive() {
                    exclusive_pas.push(pa).expect("不超过 UNMAP_CHUNK");
                }
            } else {
                panic!("unmap_and_reclaim: {va} 未映射——内核状态已损坏");
            }
        }

        // TLB flush
        let flush_va = self.vaddr + offset * PAGE_SIZE;
        let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);

        // 回收 EXCLUSIVE 帧
        for pa in &exclusive_pas {
            reclaim_exclusive_frame(*pa);
        }
        offset += n;
    }
}
```

## 所有权模型——统一的"编码到 PTE"模式

中间节点和 EXCLUSIVE 叶帧使用相同的所有权流转模式：

```
中间节点帧：
  NodeFrame::alloc() → forget (PTE 记录 paddr)
    → PageTable::drop → reclaim_children → NodeFrame::reclaim(paddr)

EXCLUSIVE 叶帧：
  AllocatedFrames::alloc_one() → forget (PTE EXCLUSIVE 位 + paddr)
    → MappedPages::drop → atomic_clear_leaf 读回 PA
    → reclaim_exclusive_frame → UnmappedFrames::from_range → Drop
```

两者的共同点：分配 → forget → PTE 编码 → 读回 PA → 重建帧 RAII → Drop 回收。

## Memory Ordering——happens-before 链

### map (Core A) → read/walk (Core B)

```
Core A (map_page):
  (1) NodeFrame::alloc() → 零初始化子表帧        [普通 store]
  (2) table.compare_exchange(idx, 0, new_pte)     [AcqRel — Release 侧]
      ↑ Release 保证 (1) 的写入对 CAS 成功后的读者可见

Core B (get_mapping / walk):
  (3) table.read_acquire(idx) → 读到 new_pte      [Acquire — 与 (2) 的 Release 配对]
  (4) 解析 pte.paddr() → 访问子表帧               [普通 load]
      ↑ (1) 的零初始化对 Core B 可见

happens-before: (1) →sb (2) →hb (3) →sb (4) ✓
```

### map (Core A) → unmap (Core B)

```
Core A (map_page):
  (1) 写入叶 PTE: compare_exchange(idx, 0, leaf_pte)  [AcqRel — Release 侧]

Core B (atomic_clear_leaf):
  (2) walk_to_leaf_location: read_acquire → 定位叶 PTE [Acquire — 与中间节点的 Release 配对]
  (3) table.swap(idx, 0)                               [AcqRel — Acquire 与 (1) 的 Release 配对]
      ↑ (1) 的 leaf_pte 对 (3) 可见

happens-before: (1) →hb (3) ✓
```

### TOCTOU 安全性

`atomic_clear_leaf` 先 `walk_to_leaf_location`（Acquire load 遍历中间节点）再 `swap(idx, 0)`。
walk 和 swap 之间不存在竞态：中间节点一旦通过 CAS 安装就**永远不会被修改或删除**
（不回收中间节点），walk 读到的路径在 swap 时仍然有效。

## 删除清单

| 删除项 | 文件 | 原因 |
|--------|------|------|
| `NodeEntry` 结构体 | `table.rs` | 无需跟踪中间节点 |
| `nodes: BTreeMap` 字段 | `table.rs` | 所有权编码在 PTE 中 |
| `root_ref_count` 字段 | `table.rs` | 不回收中间节点 |
| `ref_count_mut` / `inc_ref` / `dec_ref` | `table.rs` | 同上 |
| `unmap_page` | `table.rs` | 被 `atomic_clear_leaf` 替代 |
| `unmap_page_with_flags` | `table.rs` | 同上 |
| `unmap_at_level` | `table.rs` | 同上 |
| `unmap_at_level_with_flags` | `table.rs` | 同上 |
| `walk_create` | `table.rs` | 被 `walk_create_cas` 替代 |
| `Table::read` (Relaxed) | `lib.rs` | 统一为 `read_acquire` |
| `Table::write` (Relaxed) | `lib.rs` | 被 `compare_exchange` 替代 |
| `use alloc::collections::BTreeMap` | `table.rs:8` | `BTreeMap` 不再使用 |
| `MappedPagesInner::flags` 字段 | `mapping.rs` | 从 PTE 读取，无需缓存 |
| `MappedPagesInner::exclusive` 字段 | `mapping.rs` | PTE EXCLUSIVE 位是 source of truth |

**变更项**（非删除）：

| 变更项 | 文件 | 说明 |
|--------|------|------|
| `walk_readonly` 内部 ordering | `table.rs` | `Table::read` → `Table::read_acquire` |
| `map_page` / `map_at_level` 签名 | `table.rs` | `&mut self` → `&self`（CAS 替代锁） |
| `identity_map_range` 签名 | `table.rs` | `&mut self` → `&self` |
| `MappedPagesInner::flags()` 方法 | `mapping.rs` | 从字段读改为 PTE walk + `without_exclusive()` |
| `MappedPages` Debug 实现 | `mapping.rs` | 从 `self.0.exclusive` 改为 PTE `is_exclusive()` 查询 |
| 外层 `SpinLock` 包装 | 所有消费者 | PageTable 完全无锁 |

**新增项**：

| 新增项 | 文件 | 说明 |
|--------|------|------|
| `unsafe impl Sync for PageTable` | `table.rs` | `Arc<PageTable>` 要求 `Sync`，`NodeFrame` 含 `*mut u8` 不自动 `Sync` |

## 外部接口变更

全局机械替换 `Arc<SpinLock<PageTable>>` → `Arc<PageTable>`：

| 文件 | 变更位置 |
|------|---------|
| `crates/paging/src/mapping.rs` | `MappedPagesInner` 字段（删 flags/exclusive）、工厂方法签名、`pte_flags`/`flags()` 改为直接调 `get_mapping`、`unmap_and_reclaim` 用 `atomic_clear_leaf`、`map_alloc` 删 `lock()` 改为直接调 CAS 版 `map_page` |
| `crates/paging/src/mmio.rs` | `map_to` 签名 |
| `crates/paging/src/lib.rs` | `test_pt()` 返回 `Arc<PageTable>`、删 `Table::read`/`write` |
| `crates/memory/src/vma.rs` | `AddressSpace` 字段、`new`/`page_table()` 签名 |
| `crates/memory/src/globals.rs` | `KERNEL_PAGE_TABLE` 类型、`store`/`get` 函数 |
| `crates/memory/src/init.rs` | `init_smp` 删 lock |
| `src/main.rs` | 删 `page_table().lock()` |
| `src/boot.rs` | 删 `page_table().lock()` |
| 所有测试 | 删 `pt_ref.lock()` guard，直接调 `&self` 方法 |

## RAII 保证

改造不破坏任何 RAII 属性：

1. `MappedPages::drop()` → PTE 原子清除 + TLB flush + EXCLUSIVE 帧回收
2. `PermanentMapping::drop()` → 空操作，仅释放 Arc 引用
3. `PageTable::drop()` → `reclaim_children` 递归 walk → 所有中间节点帧归还
4. `Arc<PageTable>` 引用计数 → 最后一个引用释放时触发 PageTable drop

帧所有权流转链：

```
EXCLUSIVE 叶帧:
  AllocatedFrames → forget (PTE EXCLUSIVE)
    → atomic_clear_leaf → reclaim_exclusive_frame → Drop → 帧归还

中间节点帧:
  NodeFrame::alloc → forget (PTE 记录 paddr)
    → PageTable::drop → reclaim_children → NodeFrame::reclaim → 帧归还
```

## 与参考内核的对比

| | Linux | Theseus | Redox | **SimpleKernel (改造后)** |
|--|-------|---------|-------|-------------------------|
| map 同步 | mmap_lock + per-level lock + CAS | `&mut` 所有权 | RwLock | **CAS** |
| unmap 同步 | atomic swap | `&mut` | RwLock | **atomic swap** |
| 中间节点回收 | RCU 延迟释放 | 不回收 | 不回收 | **不回收（Drop walk）** |
| 节点跟踪 | struct page 元数据 | 无 | 无 | **无（PTE 编码）** |
| 锁数量 | N (per page) | 0 | 1 | **0** |

## 改动量估算

| 层 | 文件 | 行数 | 风险 |
|----|------|------|------|
| Table 原子操作 | `lib.rs` | ~30 | 低 |
| PageTable 重构 | `table.rs` | ~200（大量删除 + CAS 新增） | 高 |
| NodeFrameOps 扩展 | `lib.rs` | ~30 | 中 |
| MappedPagesInner | `mapping.rs` | ~50 | 中 |
| MmioRegion | `mmio.rs` | ~5 | 低 |
| memory crate | `vma.rs` + `globals.rs` + `init.rs` | ~20 | 低 |
| 内核入口 | `main.rs` + `boot.rs` | ~5 | 低 |
| 测试 | mapping + vma + table 测试 | ~80 | 中 |
| **合计** | **8 文件** | **~420（净减 ~150）** | |
