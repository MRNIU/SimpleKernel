# PageTable 内部锁重构设计

## 问题

`PageTable` 被 `Arc<SpinLock<PageTable>>` 包裹，所有操作（map/unmap/query/Drop）争同一把锁。
导致两个问题：

1. **Drop 死锁**：`MappedPages::Drop` 调用 `unmap_and_reclaim`，获取 `page_table.lock()`。
   若调用方已持有同一锁，`SpinLock` 不可重入，死锁。Rust 的隐式 drop 让这个 bug 比 C 更容易触发。
2. **SMP 并行性差**：多核并发 map/unmap 不同 VA 区域被串行化。

详细分析见 `docs/rust-rewrite/issue-pagetable-lock-granularity.md`。

## 设计决策

经对比 Linux（分层锁）、Theseus（无锁 + `&mut` 所有权）、Redox（单 RwLock），选择：

| 决策 | 选择 | 理由 |
|------|------|------|
| 锁位置 | 推入 PageTable 内部 | 外部接口从 `Arc<SpinLock<PT>>` 简化为 `Arc<PT>` |
| unmap 路径 | 完全无锁（AtomicU64 swap） | 消除 Drop 死锁 |
| map 路径 | 内部 SpinLock 保护节点分配 | 中间节点分配需互斥 |
| 中间节点回收 | 不回收（PageTable drop 时整体释放） | Theseus/Redox 验证可行，删除 ref_count 简化实现 |
| 节点存储 | `Vec<NodeFrame>`（替代 `BTreeMap<PhysAddr, NodeEntry>`） | 不回收则不需要按 paddr 查找 |
| Memory ordering | 无锁路径 Acquire/AcqRel，锁内路径保持 Relaxed | 去锁后需显式 ordering 保证跨核可见性 |

## 架构

### PageTable 结构

```rust
pub struct PageTable {
    root_paddr: PhysAddr,
    root: NodeFrame,
    /// 中间节点所有权——仅 map 路径 push，Drop 时整体释放。
    nodes: SpinLock<Vec<NodeFrame>>,
}
```

删除 `NodeEntry` 结构体、`ref_count` 字段及 `inc_ref`/`dec_ref`/`ref_count_mut` 方法。

### 方法分层

```
PageTable
├── &self 无锁方法（原子 PTE 操作）
│   ├── root_paddr()           纯读
│   ├── get_mapping(va)        walk_readonly (Acquire load)
│   └── atomic_clear_leaf(va)  walk_readonly + swap(0, AcqRel)
│
└── &self 内部锁方法（中间节点分配）
    ├── map_page(va, pa, flags)          lock nodes → walk_create → push frame → write PTE
    ├── map_at_level(va, pa, flags, lv)  同上，支持大页
    └── identity_map_range(start, end, flags)  循环调 map_at_level
```

### Table 原子操作

```rust
impl Table {
    fn read(&self, index) -> PTE          // Relaxed load（锁内路径）
    fn read_acquire(&self, index) -> PTE  // Acquire load（无锁路径）
    fn write(&self, index, pte)           // Relaxed store（锁内路径，&mut self → &self）
    fn swap(&self, index, val) -> u64     // AcqRel swap（无锁 unmap）
}
```

`write` 从 `&mut self` 改为 `&self`——`AtomicU64::store` 只需共享引用。

### atomic_clear_leaf 实现

```rust
pub fn atomic_clear_leaf(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
    // 1. 复用 walk_readonly 逻辑（Acquire load）定位叶 PTE
    //    需要新增内部辅助 walk_to_leaf_location 返回 (Table, index, level)
    //    而非现有 walk_readonly 返回 (PageTableEntry, level)
    // 2. table.swap(index, 0)——AcqRel，原子清零并返回旧值
    // 3. 解析旧 PTE，返回 PA + flags（含 EXCLUSIVE 位）
    // 不维护 ref_count，不回收中间节点
}
```

`walk_readonly` 返回已解析的 `(PageTableEntry, level)`，但 `atomic_clear_leaf` 需要
操作 PTE 所在的 `Table` 和 `index`（才能 swap）。因此新增内部辅助方法
`walk_to_leaf_location`，与 `walk_readonly` 共享遍历逻辑，但返回位置而非值。

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

不再有 `{ let mut guard = self.page_table.lock(); ... }` 作用域。

## 删除清单

| 删除项 | 文件 | 原因 |
|--------|------|------|
| `NodeEntry` 结构体 | `table.rs` | 只剩 `NodeFrame`，直接存 Vec |
| `ref_count_mut` / `inc_ref` / `dec_ref` | `table.rs` | 不回收中间节点 |
| `unmap_page` | `table.rs` | 被 `atomic_clear_leaf` 替代 |
| `unmap_page_with_flags` | `table.rs` | 同上 |
| `unmap_at_level` | `table.rs` | 同上 |
| `unmap_at_level_with_flags` | `table.rs` | 同上 |
| `MappedPagesInner::exclusive` 字段 | `mapping.rs` | PTE EXCLUSIVE 位是 source of truth |
| 外层 `SpinLock` 包装 | 所有消费者 | 锁已推入内部 |

## 外部接口变更

全局机械替换 `Arc<SpinLock<PageTable>>` → `Arc<PageTable>`：

| 文件 | 变更位置 |
|------|---------|
| `crates/paging/src/mapping.rs` | `MappedPagesInner` 字段、`map_identity`/`map_alloc`/`wrap_existing` 签名、`pte_flags` 删 lock、`unmap_and_reclaim` 删 lock |
| `crates/paging/src/mmio.rs` | `map_to` 签名 |
| `crates/paging/src/lib.rs` | `test_pt()` 返回类型 |
| `crates/memory/src/vma.rs` | `AddressSpace` 字段、`new`/`page_table()` 签名 |
| `crates/memory/src/globals.rs` | `KERNEL_PAGE_TABLE` 类型、`store_kernel_page_table`、`kernel_page_table` |
| `crates/memory/src/init.rs` | `init_smp` 删 lock |
| `src/main.rs` | 删 `page_table().lock()` |
| `src/boot.rs` | 删 `page_table().lock()` |
| 所有测试 | 删 `pt_ref.lock()` guard，直接调 `&self` 方法 |

## RAII 保证

改造不破坏任何 RAII 属性：

1. `MappedPages::drop()` → PTE 原子清除 + TLB flush + EXCLUSIVE 帧回收（路径不变，只是无锁）
2. `PermanentMapping::drop()` → 空操作，仅释放 Arc 引用（不变）
3. `PageTable::drop()` → `Vec<NodeFrame>` drop → 所有中间节点帧归还分配器
4. `Arc<PageTable>` 引用计数 → 最后一个引用释放时触发 PageTable drop

帧所有权流转链不变：
```
AllocatedFrames → MappedFrames → forget (PTE EXCLUSIVE 位)
  → atomic_clear_leaf 读回 PA + EXCLUSIVE
  → UnmappedFrames::from_range → Drop → 帧归还分配器
```

## 与 Theseus 设计的对齐

改造后更接近 Theseus：

- 页表操作无锁（Theseus 靠 `&mut Mapper`，我们靠 AtomicU64）
- 不回收中间节点（与 Theseus 一致）
- EXCLUSIVE 位追踪帧所有权（与 Theseus 一致）
- MappedPages 仿射类型 + into_permanent 转换（与 Theseus 一致）

差异：Theseus 是单地址空间 OS，靠编译期 `&mut` 保证独占；SimpleKernel 有多进程，
用 `Arc<PageTable>` + 内部锁（仅 map 路径）+ 原子 PTE 操作保证 SMP 安全。

## 改动量估算

| 层 | 文件 | 行数 | 风险 |
|----|------|------|------|
| Table 原子操作 | `lib.rs` | ~20 | 低 |
| PageTable 拆分 | `table.rs` | ~150（含大量删除） | 高 |
| MappedPagesInner | `mapping.rs` | ~40 | 中 |
| MmioRegion | `mmio.rs` | ~5 | 低 |
| paging re-export | `lib.rs` | ~5 | 低 |
| memory crate | `vma.rs` + `globals.rs` + `init.rs` | ~20 | 低 |
| 内核入口 | `main.rs` + `boot.rs` | ~5 | 低 |
| 测试 | mapping + vma 测试 | ~60 | 低 |
| **合计** | **8 文件** | **~305（净减 ~120）** | |
