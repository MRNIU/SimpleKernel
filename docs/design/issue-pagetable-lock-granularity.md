# PageTable 锁粒度与 Drop 死锁问题

> **⚠ 本文档已过时**——描述的是旧设计中的 `MappedPages` 模型。
> 当前设计使用 `OwnedPages`（见 [ADR-006](../adr/006-memory-subsystem-simplification.md)），
> Drop 不再调用 unmap（只恢复 PTE 权限），死锁场景已不存在。

## 问题描述

当前 `PageTable` 被一把 `SpinLock` 保护（`Arc<SpinLock<PageTable>>`），所有操作（map/unmap/查询/Drop）争同一把锁。这导致两个问题：

### 1. Drop 路径死锁

`MappedPages::Drop` 调用 `unmap_and_reclaim`，内部获取 `self.page_table.lock()`。如果在已持有同一页表锁的代码路径中 drop `MappedPages`，`SpinLock` 不可重入，死锁。

**显式触发：**

```rust
fn some_kernel_operation(pt_ref: Arc<SpinLock<PageTable>>) {
    let guard = pt_ref.lock();          // 第一次获取锁
    let mp = MappedPages::map_identity(pt_ref.clone(), pa, 1, flags)
        .expect("map");
    drop(mp);                           // Drop → lock() → 死锁
    drop(guard);
}
```

**隐式触发（更隐蔽）：**

```rust
fn update_mapping(pt_ref: Arc<SpinLock<PageTable>>) {
    let mut guard = pt_ref.lock();
    let old_mp: MappedPages = take_old_mapping();
    let new_mp = create_new_mapping(&mut guard);
    store_new_mapping(new_mp);
    // old_mp 在函数末尾隐式 drop，但 guard 还没释放 → 死锁
}
```

Rust 的隐式 drop（变量离开作用域）让这个 bug 比 C 中更容易出现——不需要写 `drop(mp)` 就能触发。

### 2. SMP 并行性差

多核并发 map/unmap 不同虚拟地址区域被串行化，因为它们争同一把锁。

## 涉及代码

| 文件 | 位置 | 说明 |
|------|------|------|
| `crates/paging/src/mapping.rs` | `MappedPagesInner::unmap_and_reclaim` | Drop 路径，获取 `self.page_table.lock()` |
| `crates/paging/src/mapping.rs` | `MappedPages::map_identity/map_alloc` | 构造路径，获取 `pt_ref.lock()` |
| `crates/paging/src/mapping.rs` | `MappedPagesInner::pte_flags` | 查询路径，获取 `self.page_table.lock()` |
| `crates/paging/src/mapping.rs` | `MappedPagesInner::as_type_mut` | 间接获取锁（调用 `pte_flags`） |
| `crates/paging/src/table.rs` | `PageTable` | PTE 已使用 `AtomicU64` 存储 |
| `crates/paging/src/lib.rs` | `Table::read/write` | 使用 `Ordering::Relaxed` 原子操作 |

## 解决方案

### 方案 C（推荐）：unmap 改为原子操作，消除 Drop 死锁

**核心思路**：PTE 已经是 `AtomicU64`，unmap 操作（将 PTE 置零并读回旧值）天然是原子的，不需要页表锁。

**改造范围：**

1. 新增 `PageTable::atomic_unmap(va) -> Option<(PhysAddr, PteFlags)>`
   - walk 是只读的（中间节点在 map 时分配，不会被并发回收）
   - 用 `AtomicU64::swap(0, Ordering::AcqRel)` 原子清除 PTE
   - 不需要获取 `SpinLock`

2. `MappedPagesInner::unmap_and_reclaim` 改用 `atomic_unmap`
   - Drop 路径不再获取页表锁 → 死锁消失

3. `map_page` / `identity_map_range` 保持使用 `SpinLock`
   - 分配中间节点需要互斥（两个 map 可能竞争创建同一中间节点）
   - 或者也改为 CAS 竞争：CAS 设置中间节点指针，失败方释放多余分配

**注意事项：**
- walk_readonly 假设中间节点不会被并发释放——当前 `unmap_at_level_with_flags` 会回收空的中间节点（引用计数归零时），这与无锁 unmap 冲突
- 最简方案：atomic_unmap 不回收中间节点（只清叶子 PTE），中间节点泄漏但安全。后续通过惰性回收或批量清理解决
- TLB flush 时序不变：先原子清 PTE → flush TLB → 回收帧

### 方案 A（长期）：全无锁页表（Linux 5.x 做法）

- map/unmap 都用 CAS 原子操作
- 中间节点回收需要 RCU 或延迟释放
- 实现复杂度高，SMP 性能最好
- 等到 SMP 性能成为瓶颈时再考虑

### 方案 B（备选）：per-subtree 分段锁

- 按 L2 顶级条目分 512 把锁
- 不同虚拟地址区域可并行
- 实现简单但不彻底解决 Drop 死锁（同一 subtree 内仍可能）

## 参考

- Linux 内核 `__pte_update`/`ptep_get_and_clear` — 原子 PTE 操作
- Theseus `MappedPages` — 使用类似的 RAII 模型但无锁页表操作
- 当前 `Table::read/write` 已使用 `AtomicU64`（`crates/paging/src/lib.rs:189-202`）
