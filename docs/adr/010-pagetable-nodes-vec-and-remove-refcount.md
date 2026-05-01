# ADR-010: PageTable 内部结构简化——BTreeMap → Vec，删除引用计数死代码

> **状态**: 已接受
>
> **日期**: 2026-04-17
>
> **审计阶段**: R3 — 内存子系统（回看）
>
> **涉及模块**: `crates/paging`

## 背景

[ADR-007](007-eliminate-vma-and-dead-code.md) 删除了 `PageTable::unmap_page` / `unmap_page_with_flags` / `dec_ref`——SAS 全量映射下无 unmap 语义，PTE 只建不删。

遗留代码：

```rust
// crates/paging/src/table.rs
struct NodeEntry {
    _frame: AllocatedFrames,
    ref_count: u16,               // ← 只写不读
}

pub struct PageTable {
    root: AllocatedFrames,
    root_ref_count: u16,           // ← 只写不读
    nodes: BTreeMap<PhysAddr, NodeEntry>,   // ← key 能力无用户
}

impl PageTable {
    fn ref_count_mut(&mut self, paddr: PhysAddr) -> &mut u16 { ... }  // 仅被 inc_ref 调用
    fn inc_ref(&mut self, paddr: PhysAddr) { *self.ref_count_mut(paddr) += 1; }
}
```

### 问题 1：`ref_count` 字段是纯死代码

全 crate 扫描：

| 操作 | 位置 | 说明 |
|------|------|------|
| 写入（初始化为 0） | `PageTable::create`, `walk_create` 中 `NodeEntry { ref_count: 0, ... }` | 初始化 |
| 写入（递增） | `inc_ref → ref_count_mut += 1` | `walk_create` 新建中间节点时；`create_pte` 新建叶 PTE 时 |
| **读取** | **无** | `ref_count_mut` 返回 `&mut u16` 但只被 `inc_ref` 调用，调用者不读返回值 |

该字段从未被任何逻辑读取。移除后无行为变化。

### 问题 2：`BTreeMap<PhysAddr, NodeEntry>` 的 key 能力失去用户

BTreeMap 的价值在于 **按 key 查找 / 删除**。删除 `dec_ref` 后，`nodes` 的使用场景只有：

| 操作 | 代码位置 | 使用到的能力 |
|------|---------|-------------|
| `insert(frame_paddr, NodeEntry {...})` | `walk_create` | 仅"添加"，不使用有序性 |
| `get_mut(&paddr)` | `ref_count_mut`（上文确定只被 `inc_ref` 调用） | 按 key 查找，但目标是 ref_count 字段（死代码） |
| 整体 `Drop`（隐式） | `PageTable` drop 时 | 遍历所有 entry 自动 drop |

删除 `ref_count` 后，`nodes` 只需要 "添加" 和 "遍历 drop"——`Vec<AllocatedFrames>` 足够。

### 问题 3：`NodeEntry` 结构体成为单字段包装器

移除 `ref_count` 后 `NodeEntry` 只剩 `_frame: AllocatedFrames`，相当于 `AllocatedFrames` 本身。结构体可消解。

## 量化

当前 `NodeEntry` 每个条目：`AllocatedFrames`（16 字节 FrameSpan）+ `u16` ref_count + BTreeMap 节点开销（指针、颜色位等，约 48 字节 per entry）。

`Vec<AllocatedFrames>`：每个条目 16 字节 + 分摊的 Vec 容量开销（可忽略）。

内存节省 ~50 字节/节点。主要收益不是内存而是**可读性**——内部结构与实际职责（持有所有权）对齐。

## 备选方案

### 方案 A: 删除 ref_count 体系 + `BTreeMap` → `Vec<AllocatedFrames>`

```rust
pub struct PageTable {
    root: AllocatedFrames,
    nodes: Vec<AllocatedFrames>,   // 仅持有所有权
}

impl PageTable {
    fn walk_create(&mut self, va: VirtAddr) -> Result<(PhysAddr, usize), PagingError> {
        let mut paddr = self.root.start_paddr();
        for level in (1..PT_LEVELS).rev() {
            let mut table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                let frame = crate::alloc_node_frame()?;
                let frame_paddr = frame.start_paddr();
                self.nodes.push(frame);        // O(1) 摊还
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                paddr = frame_paddr;
                // 不再 inc_ref
            } else if pte.is_leaf(level) {
                panic!(...);
            } else {
                paddr = pte.paddr();
            }
        }
        let idx = vpn_index(va, 0);
        Ok((paddr, idx))
    }
    // create_pte 同样删除 inc_ref 调用
}
```

**优点**:
- 删除 `NodeEntry` struct、`ref_count` / `root_ref_count` 字段、`ref_count_mut` / `inc_ref` 方法
- 净减 ~30 行代码 + 消除一份"写而不读"的 mental load
- Vec 的 push 摊还 O(1)，优于 BTreeMap 的 O(log n) insert
- 未来若真需要按 paddr 查找（例如为 H6 无锁 walk 做缓存），可**基于实际需求**重新引入合适的数据结构（HashMap / 原地 BTreeSet 等），而非继续使用当前这个被阉割的 BTreeMap

**缺点**:
- 失去按 paddr 查找中间节点的能力——但当前无调用方需要此能力
- 未来若重新引入 unmap（语义上需要"通过 paddr 找到 NodeEntry 并 dec_ref"），需要重建容器；但按 ADR-006/007 方向，SAS 全量映射下不会重新引入 unmap

### 方案 B: 保留 BTreeMap，仅删除 ref_count

**优点**:
- `NodeEntry` 简化但保留；未来如需按 paddr 查找不用改容器

**缺点**:
- 保留了当前无用户的 key 能力
- `NodeEntry` 成为 `struct NodeEntry(AllocatedFrames)` 单字段包装——徒增抽象层
- BTreeMap 每节点 ~48 字节额外开销

### 方案 C: 保持现状

**优点**: 零改动

**缺点**: 死代码持续存在，下次阅读的人再理解一次"为什么有 ref_count"

## 决策

选择 **方案 A**。

## 理由

- **方案 C 保留无收益的复杂度**
- **方案 B 的"未来可能用到"论证不成立**：ADR-006/007 已确立不引入 unmap；若真需要，届时基于具体需求设计
- **方案 A 与代码实际职责对齐**：`nodes` 的职责是"持有中间页表节点的所有权"，`Vec<AllocatedFrames>` 精确表达此职责，无额外干扰概念

**Rust 范式层面**：这是对 "类型应反映实际用途" 的承认。`BTreeMap<K, V>` 暗示"按 key 查找/删除"，使用时读者会据此推理；但实际没有查找/删除，就是语义噪声。`Vec<AllocatedFrames>` 直白地说"我是个帧所有权列表"。

## 影响

### 代码变更

| 文件 | 变更 |
|------|------|
| `crates/paging/src/table.rs` | 删除 `struct NodeEntry`、`root_ref_count` 字段、`ref_count_mut` 方法、`inc_ref` 方法；`nodes` 字段类型改为 `Vec<AllocatedFrames>`；`walk_create` 中 `self.nodes.insert(...)` 改为 `self.nodes.push(frame)`；删除 `self.inc_ref(paddr)` 调用（共 2 处：walk_create 和 create_pte）；`PageTable::create` 初始化字段调整 |
| `crates/paging/src/table.rs` | 模块内 `use alloc::collections::BTreeMap` 改为 `use alloc::vec::Vec` |

### API 变更

| 项 | 变更 |
|----|------|
| `PageTable` 公开 API（`create`, `root_paddr`, `create_pte`, `update_flags`, `get_mapping`, `identity_map_range`） | 不变 |

外部调用方完全不受影响。

### 测试

- 所有 paging-test 无需修改——测试的是 PageTable 的公共行为，不涉及内部字段
- 无新增测试需求

### 文档

- `crates/paging/src/table.rs` 模块 doc comment：`NodeEntry` 说明删除；`nodes` 字段注释更新为"仅持有中间页表节点所有权"
- `docs/design/memory-subsystem-v2.md` §8.1：`PageTable` 结构定义更新

## 参考

- [ADR-007](007-eliminate-vma-and-dead-code.md) — 删除 `unmap_page` / `dec_ref`，本 ADR 清理遗留副产物
- [Rust API Guidelines §C-STRUCT-PRIVATE](https://rust-lang.github.io/api-guidelines/future-proofing.html) — 数据结构应反映实际用途
- [Linux `kernel/mm/memory.c` pgtable allocation](https://github.com/torvalds/linux/blob/master/mm/memory.c) — Linux 通过 PMD/PUD entry 的 `_count` 字段追踪引用，因为 Linux 有 unmap；SimpleKernel 无 unmap 故无需引用计数
