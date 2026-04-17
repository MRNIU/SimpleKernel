# ADR-012: PageTable 拆分；hot-path PTE 更新无锁化

> **状态**: 提议
>
> **日期**: 2026-04-17
>
> **审计阶段**: R3 — 内存子系统（回看）
>
> **涉及模块**: `crates/paging`

## 背景

当前 `KERNEL_PAGE_TABLE` 为 `spin::Once<sync_crate::SpinLock<PageTable>>`——全局单锁保护整个 PageTable。所有路径（建立 PTE、修改 PTE flags、只读查询）都竞争这把锁。

**hot path 是 `OwnedPages::{new, set_flags, Drop}`**——这三条路径调用 `batch_update_flags → update_flags`，操作的是**已经存在的 PTE**（因为 boot 期 `identity_map_range` 已经建立了全量背景映射）。`update_flags` 本身不分配中间节点、不修改 `PageTable.nodes` 容器、不修改页表骨架——它只改单个叶 PTE 的 flags 字段。

**slow path（boot/MMIO）**：`identity_map_range` 通过 `set_page_flags → walk_create` 建立新 PTE，可能分配中间节点。

当前设计让这两条路径共享同一把锁：
- hot path 的多核并发被串行化（典型场景：多核同时构造 OwnedPages / 修改权限 / Drop）
- 在未来引入用户进程或并行 DMA 之后，单锁成瓶颈

而基础设施已经到位：PTE 存储本身是 `AtomicU64`（`crates/paging/src/table.rs::Table`），单 PTE 读写天然原子。

## SAS 架构下的额外简化

[ADR-007](007-eliminate-vma-and-dead-code.md) 确立 **"PTE 只建不删"**——SAS 全量映射模型下没有 `unmap` 路径，PTE 的生命周期是 "创建 (set_page_flags) → 修改 flags (update_flags) → 永远存在"。

这消除了通用无锁页表设计中最难的问题——**"PTE 读取与 PTE 删除的竞态"**：
- Linux 用 RCU 延迟释放中间页表节点，避免 walker 读到已 freed 的节点
- Theseus 的 `MappedPages` 通过 unmap 回收中间节点，需要精细的同步

SimpleKernel 没有 unmap，意味着：
- 中间页表节点一旦建立就**永久有效**——无 dangling 引用风险
- `walk_to_leaf`（只读遍历）可以完全无锁执行，读到的 PTE 要么是"尚未建立"（无效）要么是"永久存在"（有效）
- 单 PTE 更新通过 `AtomicU64::swap` 原子完成

## 现行原子 ordering 的不足

当前 `Table::read/write` 使用 `Ordering::Relaxed`：

```rust
// crates/paging/src/table.rs
fn read(&self, index: usize) -> PageTableEntry {
    let val = unsafe { (*self.base.add(index)).load(Ordering::Relaxed) };
    PageTableEntry::from_raw(val)
}

fn write(&mut self, index: usize, pte: PageTableEntry) {
    unsafe { (*self.base.add(index)).store(pte.as_raw(), Ordering::Relaxed) };
}
```

在全锁保护下 Relaxed 正确——`SpinLock::{lock, unlock}` 提供 Acquire/Release 跨 CPU 可见性。但无锁化 hot path 后，Relaxed **不足**：

- **场景 A**：CPU0 在 `walk_create` 中分配新中间帧、零填充、写父 PTE 指向它；CPU1 并发 `walk_to_leaf` 读父 PTE。
  - Relaxed 下 CPU1 可能读到新父 PTE 的值，但尚未读到中间帧被零填充——descend 后读到未初始化内容
  - 需要父 PTE 写入为 **Release**，子 PTE 读为 **Acquire**——配对传递"零填充已完成"的 happens-before
- **场景 B**：CPU0 `update_flags` swap 新 flags，同时 CPU1 `walk_to_leaf` 读该 PTE
  - `AtomicU64::swap(_, AcqRel)` 保证读写都看到一致状态

## 参考 Linux 的 PTE 访问模式

Linux 的 `pte_clear` / `ptep_get_and_clear` 使用 `READ_ONCE` / `WRITE_ONCE`（等价于 Relaxed）+ 外层 page table lock（spinlock）。对 update-path 依赖外层锁与 SimpleKernel 现状相同。

Linux 在**某些 lockless walker** 路径（GUP-fast、fast mremap）使用 `ptep_get_lockless`，本质是 `READ_ONCE` + 全局 memory barrier（`smp_rmb`）。SimpleKernel 由于无 unmap，不需要 RCU 或 generation counter——直接 Release/Acquire 足够。

## 备选方案

### 方案 A: PageTable 拆分 + hot-path 无锁 + ordering 升级

**结构调整**：

```rust
// crates/paging/src/table.rs
pub struct PageTable {
    /// 根帧——create 后永不变动，无需同步保护
    root: AllocatedFrames,
    /// 中间节点容器——仅 walk_create（建 PTE）路径需要互斥保护
    nodes: sync_crate::SpinLock<Vec<AllocatedFrames>>,
}

impl PageTable {
    pub fn create() -> Result<Self, PagingError> {
        let root = crate::alloc_node_frame()?;
        Ok(Self {
            root,
            nodes: SpinLock::new(Vec::new(), "pt_nodes", lock_level::KERNEL_PT),
        })
    }

    pub fn root_paddr(&self) -> PhysAddr { self.root.start_paddr() }  // 无锁

    /// 无锁只读 walk——SAS 下 PTE 只建不删，walker 不会读到悬挂
    fn walk_to_leaf(&self, va: VirtAddr) -> Option<(PageTableEntry, PhysAddr, usize, usize)> {
        let mut paddr = self.root.start_paddr();
        for level in (0..PT_LEVELS).rev() {
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);  // Ordering::Acquire
            if !pte.is_valid() { return None; }
            if pte.is_leaf(level) { return Some((pte, paddr, idx, level)); }
            paddr = pte.paddr();
        }
        None
    }

    /// 只改 flags，不分配中间节点——无锁
    pub fn update_flags(&self, va: VirtAddr, new_flags: PteFlags) -> Result<PteFlags, PagingError> {
        let (pte, paddr, idx, level) = self.walk_to_leaf(va).ok_or(PagingError::PageNotMapped)?;
        let leaf_flags = new_flags.for_leaf_at_level(level);
        let new_pte = PageTableEntry::new(pte.paddr(), leaf_flags);
        let atomic = unsafe { &*(paddr.as_usize() as *const AtomicU64).add(idx) };
        // SAFETY: paddr 源自 self.root 或 self.nodes 持有的帧——生命周期由 &self 保证
        let old_raw = atomic.swap(new_pte.as_raw(), Ordering::AcqRel);
        Ok(PageTableEntry::from_raw(old_raw).flags())
    }

    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {  // 无锁
        let (pte, _, _, level) = self.walk_to_leaf(va)?;
        let page_size = crate::page_size_at_level(level);
        let offset = va.as_usize() & (page_size - 1);
        Some((pte.paddr() + offset, pte.flags()))
    }

    /// 建新 PTE——仅此路径需要锁
    pub fn set_page_flags(&self, va: VirtAddr, pa: PhysAddr, flags: PteFlags) -> Result<(), PagingError> {
        // 快速路径：先无锁检查 PTE 是否已存在
        if let Some((pte, _, _, level)) = self.walk_to_leaf(va) {
            if pte.paddr() != pa { panic!(...); }
            let leaf_flags = flags.for_leaf_at_level(level);
            if pte.flags() == leaf_flags { return Ok(()); }     // 幂等
            return Err(PagingError::FlagsConflict);             // 与 ADR-011 一致
        }
        // 慢速路径：建立新 PTE 可能分配中间节点——取锁
        let mut nodes = self.nodes.lock();
        self.walk_create_and_write(&mut nodes, va, pa, flags)
    }

    pub fn identity_map_range(&self, start: PhysAddr, end: PhysAddr, flags: PteFlags) {
        // 不变（调用 set_page_flags）
    }
}
```

**ordering 升级**：

```rust
fn read(&self, index: usize) -> PageTableEntry {
    let val = unsafe { (*self.base.add(index)).load(Ordering::Acquire) };
    PageTableEntry::from_raw(val)
}

fn write(&mut self, index: usize, pte: PageTableEntry) {
    unsafe { (*self.base.add(index)).store(pte.as_raw(), Ordering::Release) };
}

// 或直接替换 Table::write 为专用 swap 路径（见 update_flags）
```

**API 改动**：
- `&mut self` → `&self`：因内部锁从外部迁移到 `nodes` 字段
- 所有 `PageTable` 方法变为 `&self`，消除调用方的 `Mut` 借用检查摩擦

**优点**:
- hot path（OwnedPages::{new, set_flags, Drop}）完全无锁——多核并发不互相阻塞
- `update_flags` / `get_mapping` 无锁执行
- slow path（新建 PTE）仍受锁保护，保证 walk_create 的中间节点分配互斥
- `kernel_page_table()` 返回类型从 `&SpinLock<PageTable>` 改为 `&PageTable`——调用方无需显式 lock
- ordering 升级（Release/Acquire）使 PTE 访问语义显式、符合 Linux 约定

**缺点**:
- 改动面较大——所有 `kernel_page_table().lock()` 调用点需要移除 `.lock()`
- 需要多核并发压力测试验证无锁路径正确性
- `PageTable` 内部字段从简单 `Vec` 变为 `SpinLock<Vec>`，增加一层

### 方案 B: 保持单锁

**优点**: 零改动

**缺点**:
- SMP hot path 串行化
- 用户进程引入后单锁成瓶颈
- 未来再拆分时改动面同样大（甚至更大，若届时已有更多调用方）

### 方案 C: Per-level 锁（Linux 风格 split page table lock）

每个中间节点持自己的锁。

**优点**: 细粒度并发

**缺点**:
- 锁分配开销（每个节点一把锁）
- 锁序复杂——多路径锁顺序冲突难处理
- Linux 因 RCU / unmap / TLB shootdown 复杂性采用；SimpleKernel 无 unmap，没必要

## 决策

选择 **方案 A**。

## 理由

- **方案 B 维持性能天花板**，用户进程引入后必改
- **方案 C 过度设计**——Linux 的分锁设计为应对 unmap、RCU、TLB shootdown；SimpleKernel SAS 全量映射下 walker 无需任何同步
- **方案 A 利用 SAS 结构优势**：SAS 的"PTE 只建不删"特性让无锁 walk 安全，这是 Linux 不具备的简化条件
- **成本主要是一次性重构**，获得的是长期 SMP 扩展性

**Rust 范式层面**：这是对 "fine-grained interior mutability" 的应用。内部锁从 `SpinLock<PageTable>`（整个对象被锁）下沉到 `SpinLock<Vec<AllocatedFrames>>`（仅保护可变部分），让 `PageTable` 的不可变部分（root）和原子部分（PTE 数组）自由共享——与 Rust 的 `&self` / 借用模型更匹配。

## 影响

### 代码变更

| 文件 | 变更 |
|------|------|
| `crates/paging/src/table.rs` | 重写 `PageTable` 结构——`nodes` 改为 `SpinLock<Vec<AllocatedFrames>>`；方法签名 `&mut self` → `&self`；`walk_to_leaf` 改为 `&self` 无锁；`walk_create` 拆分为 `walk_create_and_write(nodes: &mut Vec<_>)` 内部方法；`update_flags` 改为 atomic swap 无锁 |
| `crates/paging/src/table.rs` | `Table::read/write` Ordering 从 `Relaxed` 升级为 `Acquire` / `Release`；新增 `Table::swap(idx, new) -> PageTableEntry` 用 `AcqRel` |
| `crates/paging/src/lib.rs` | `KERNEL_PAGE_TABLE: spin::Once<PageTable>`（去掉外层 `SpinLock`）；`kernel_page_table()` 返回 `&'static PageTable`；`init_kernel_page_table(pt)` 签名不变 |
| `crates/paging/src/mapping.rs` | `claim_pages` / `batch_update_flags`（若 ADR-009 删除 claim_pages，则仅 batch_update_flags）去掉 `kernel_page_table().lock()`；直接调 `kernel_page_table().update_flags(va, flags)` |
| `crates/paging/src/mmio.rs` | `MmioRegion::map` 去掉 `lock()` 调用 |
| `crates/memory/src/init.rs` | 同上——`identity_map_range` 不再需要 `guard` |

### API 变更

| 项 | 变更 |
|----|------|
| `PageTable::set_page_flags`, `update_flags`, `get_mapping`, `identity_map_range`, `root_paddr` | `&mut self` → `&self` |
| `kernel_page_table()` 返回类型 | `&'static SpinLock<PageTable>` → `&'static PageTable` |
| `PagingError::FlagsConflict` | 与 ADR-011 一致（本 ADR 与 ADR-011 在 `set_page_flags` 返回值上保持一致） |

### 测试

- **现有 paging-test 需要适配**：调用 `PageTable::set_page_flags(&mut pt, ...)` 改为 `PageTable::set_page_flags(&pt, ...)`；test 中的 `let mut pt` 可以改为 `let pt`
- **新增多核并发测试**：在 `tests/paging-test/` 下新建 `concurrent.rs` 二进制——多 CPU 同时做 `update_flags` 与 `get_mapping`，验证无 torn read、无 panic
- **测试 Ordering 正确性**：手工构造"一核建新 PTE + 另一核读"场景，跑足够多次无 flaky

### 文档

- `crates/paging/src/table.rs` 模块 doc：新增"锁粒度与无锁路径"小节
- `crates/paging/src/lib.rs` 模块 doc：`KERNEL_PAGE_TABLE` 类型改变后的说明
- `docs/design/memory-subsystem-v2.md` §8.1 / §8.4：`PageTable` 结构定义更新
- `docs/design/issue-pagetable-lock-granularity.md`：标注"已被 ADR-012 解决"（该文档已标过时，本 ADR 进一步确认）

## 参考

- [ADR-007](007-eliminate-vma-and-dead-code.md) — "PTE 只建不删"的确立，是本 ADR 无锁化的前提
- [Linux `ptep_get_lockless`](https://github.com/torvalds/linux/blob/master/include/linux/pgtable.h) — 无锁 PTE 读取模式参考
- [Linux `pte_offset_map_lock` / `pte_unmap_unlock`](https://github.com/torvalds/linux/blob/master/mm/memory.c) — per-table 锁参考（SimpleKernel 不采用，因无 unmap 需求）
- [C11 memory model §5.1.2.4](https://open-std.org/JTC1/SC22/WG14/www/docs/n1570.pdf) — Release/Acquire 语义的形式化定义
- [Rust nomicon: atomics](https://doc.rust-lang.org/nomicon/atomics.html) — Rust 中原子 ordering 的实践指南
- [Theseus `MappedPages`](https://github.com/theseus-os/Theseus/tree/theseus_main/kernel/memory) — 使用 unmap 的 RAII 模型（SimpleKernel 因无 unmap 不适用）
- `docs/design/issue-pagetable-lock-granularity.md` — 早期已识别单锁问题，本 ADR 给出最终方案
