# page_table

架构无关的多级页表——walk / map / unmap 逻辑与中间帧自动管理。

## 概览

`page_table` 实现了多级页表的通用遍历、映射和解映射逻辑，
PTE 编解码由 `page_table_entry` crate 提供，帧分配通过 `NodeFrameOps` trait 注入。
这种分层使得同一份 walk 代码在裸机和宿主机测试中均可运行。

核心类型 `PageTable<F>` 拥有根帧和所有中间帧的所有权，
通过引用计数在 unmap 时自动回收空的中间节点，drop 时归还所有帧。

## 核心设计

### `NodeFrameOps` trait

```rust
pub trait NodeFrameOps: Send + Sized {
    fn alloc() -> Result<Self, PageTableError>;
    fn paddr(&self) -> PhysAddr;
}
```

消费方通过实现此 trait 注入帧分配能力：
- **裸机**：`impl NodeFrameOps for AllocatedFrames`（在 `memory` crate 中）
- **测试**：`impl NodeFrameOps for HeapNodeFrame`（本 crate 内置，`test-support` feature）

上层通过类型别名隐藏泛型参数：`type PageTable = page_table::PageTable<AllocatedFrames>;`

### `PageTable<F>` 内部结构

```rust
pub struct PageTable<F: NodeFrameOps> {
    root_paddr: PhysAddr,               // 根帧物理地址（写入 satp / TTBR）
    root: F,                            // 根帧所有权
    frames: BTreeMap<PhysAddr, F>,      // 中间帧——以物理地址为键
    ref_counts: BTreeMap<PhysAddr, u16>,// 每帧中有效 PTE 数
}
```

### 引用计数

每个页表帧（含根帧）维护一个计数器，记录其中有效 PTE 的数量：
- `map` 时 +1，`unmap` 时 -1
- `unmap` 回溯时：count == 0 且非根帧 → 清除上级 PTE 并回收帧
- 避免了 O(512) 全扫描判断帧是否为空

设计参考 Linux `free_pgtables()` + `struct page::_mapcount`。

## 操作流程

### map（映射）

```
map_page(va, pa, flags)  或  map_at_level(va, pa, flags, level)
  │
  ├─→ walk_create(va, target_level)     ← 从根到目标层，按需分配中间节点
  │     ├─→ PTE 无效 → alloc() + 写入中间 PTE + inc_ref
  │     ├─→ PTE 是大页叶 → HugePageConflict
  │     └─→ PTE 是中间节点 → 继续向下
  │
  ├─→ 检查目标 PTE 是否已映射 → AlreadyMapped
  │
  └─→ 写入叶 PTE（flags.for_leaf_at_level） + inc_ref
```

### unmap（解映射）

```
unmap_page(va)  或  unmap_at_level(va, level)
  │
  ├─→ 从根到目标层记录路径栈 [(parent_paddr, idx, child_paddr)]
  │
  ├─→ 清除叶 PTE + dec_ref
  │
  └─→ 沿路径栈回溯：
        count == 0 → 清除上级 PTE + 回收帧 + dec_ref（继续回溯）
        count > 0  → 停止
```

### identity_map_range（范围 identity 映射）

```
identity_map_range(start, end, flags)
  │
  ├─→ 第一阶段：收集 (va, pa, level) ← 自动选择最大页大小
  │     1GB → 2MB → 4KB
  │
  └─→ 第二阶段：逐个 map_at_level，失败时逆序 unmap 回滚
```

两阶段确保事务性——全部成功或全部回滚。

## 模块结构

```
src/
├── lib.rs           NodeFrameOps trait、HeapNodeFrame、Table、LevelInfo、常量
├── table.rs         PageTable<F> 的 walk / map / unmap / get_mapping / identity_map_range
├── error.rs         PageTableError 枚举
└── tests/
    ├── mod.rs       测试模块入口
    ├── level.rs     层级参数测试（shift、index、page_size）
    └── table.rs     功能测试（map/unmap/huge page/range/refcount/rollback）
```

## 使用示例

### 基本映射与查询

```rust
use page_table::{PageTable, PteFlags, PteFlagsOps};
use address::{PhysAddr, VirtAddr};

let mut pt = PageTable::<HeapNodeFrame>::create()?;

// 映射 4KB 页
pt.map_page(
    VirtAddr::new(0x1000),
    PhysAddr::new(0x8020_0000),
    PteFlags::kernel_rw(),
)?;

// 查询
let (pa, flags) = pt.get_mapping(VirtAddr::new(0x1000)).unwrap();
assert_eq!(pa, PhysAddr::new(0x8020_0000));
assert!(flags.is_writable());

// 解映射
let old_pa = pt.unmap_page(VirtAddr::new(0x1000))?;
assert_eq!(old_pa, PhysAddr::new(0x8020_0000));
```

### 大页映射

```rust
// 2MB 大页（level 1）
pt.map_at_level(
    VirtAddr::new(0x20_0000),
    PhysAddr::new(0x4000_0000),
    PteFlags::kernel_rw(),
    1, // level
)?;

// 查询大页内部偏移
let (pa, _) = pt.get_mapping(VirtAddr::new(0x20_0100)).unwrap();
assert_eq!(pa, PhysAddr::new(0x4000_0100)); // 偏移 0x100
```

### 范围 identity 映射

```rust
// 自动选择最大页大小（1GB / 2MB / 4KB）
pt.identity_map_range(
    PhysAddr::new(0x8000_0000),
    PhysAddr::new(0x8030_0000),
    PteFlags::kernel_rw(),
)?;
```

## 错误类型

| 错误 | 含义 | 触发场景 |
|------|------|----------|
| `AllocationFailed` | 帧分配失败 | 分配器未初始化或帧耗尽 |
| `AlreadyMapped` | VA 已映射 | 重复映射同一 VA |
| `HugePageConflict` | walk 路径上遇到大页 | 在大页覆盖范围内映射小页 |
| `PageNotMapped` | VA 未映射 | unmap 未映射的地址 |
| `InvalidRange` | 范围无效 | `start >= end` |

## 注意事项

### 1. TLB 刷新由调用方负责

所有 map / unmap 操作后，调用方**必须**执行架构相关的 TLB 维护
（RISC-V `sfence.vma` / AArch64 `TLBI` + `DSB` + `ISB`）。
页表本身不触发 TLB 操作。

### 2. 不支持大页分裂

当前不能 unmap 大页的一部分，也不能在大页覆盖范围内映射小页。
如需部分 unmap，须先手动将大页分裂为小页再操作。

### 3. 原子访问与外部同步

`Table` 内部使用 `AtomicU64`（Relaxed ordering）保证单个 PTE 不会 torn read/write，
但整体并发安全需要外层 `SpinLock` 提供。Relaxed 即可，
因为 SpinLock 的 acquire/release 语义已经提供了必要的 memory barrier。

## Feature Flags

| Feature | 作用 | 使用场景 |
|---------|------|----------|
| `test-support` | 导出 `HeapNodeFrame`、禁用 `no_std` | 下游 crate 的 `[dev-dependencies]` |

### `test-support` 的 Cargo.toml 配置

**必须通过 `[dev-dependencies]` 启用，不能放在 `[dependencies]`。**
该 feature 依赖 `alloc` 的宿主机堆分配器。如果放在 `[dependencies]` 中，
裸机交叉编译时虽然不会直接失败（因为 kernel 本身有 `#[global_allocator]`），
但会将测试专用的 `HeapNodeFrame` 编入内核二进制，增加无用代码。

```toml
# ✅ 正确
[dev-dependencies]
page_table = { path = "../page_table", features = ["test-support"] }

# ❌ 错误——测试代码会编入裸机二进制
[dependencies]
page_table = { path = "../page_table", features = ["test-support"] }
```

## TODO

### 大页分裂（THP splitting）

引入 transparent huge page splitting 支持，允许在大页覆盖范围内
映射小页或部分 unmap。需要在 walk 路径上检测大页并自动分裂为下一级页表。

### SMP 细粒度锁

当前依赖外层 `SpinLock` 全局互斥。可参考 Linux 的 split page table lock
（每帧一把锁）降低锁争抢，但需要更精细的引用计数同步。
