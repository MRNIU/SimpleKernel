<!-- Copyright The SimpleKernel Contributors -->

# ADR-011: MMIO overlap 检测统一至 `create_pte`；引入 `FlagsConflict` 错误

> **状态**: 已接受
>
> **日期**: 2026-04-17
>
> **审计阶段**: R3 — 内存子系统（回看）
>
> **涉及模块**: `crates/memory`, `crates/paging`

## 背景

[ADR-007](007-eliminate-vma-and-dead-code.md) 删除 VMA 时引入了 ~30 行 MMIO 跟踪机制：

```rust
// crates/memory/src/lib.rs
static MMIO_REGIONS: SpinLockIrq<BTreeMap<VirtAddr, usize>> = SpinLockIrq::new(
    BTreeMap::new(),
    "mmio_regions",
    sync_crate::lock_level::UNSPECIFIED,  // 锁级别语义错误（非诊断性质）
);

pub fn map_mmio(paddr, size) -> Result<VirtAddr, MemoryError> {
    let region = MmioRegion::map(paddr, size)?;    // ← 先修改 PTE
    let region_base = region.base();
    let region_size = region.size();
    match check_mmio_overlap(region_base, region_size) {  // ← 再检查重叠
        Ok(()) => {}
        Err(MmioIdentical) => {}
        Err(e) => panic!("MMIO 区域注册失败: {e}"),  // ← panic 时 PTE 已被修改
    }
    Ok(paddr.to_virt())
}
```

### 问题 1：操作顺序颠倒导致部分重叠时 PTE 损坏后 panic

调用 `map_mmio(0x1000_0000, 0x1000)` 后再调 `map_mmio(0x1000_0800, 0x1000)`：
- 第二次：`MmioRegion::map` 按页对齐扩展到 `[0x1000_0000, 0x1000_2000)`，**新建 `0x1000_1000` 的 PTE 映射**（之前未映射）
- `check_mmio_overlap` 发现部分重叠 → panic
- 此时 PTE `0x1000_1000` 已被作为 Device 内存映射，但 panic 中止了注册——PTE 与注册表状态不一致

panic 后内核 halt 虽不产生"后果"，但代码逻辑"先做不可回滚动作再检查能否做"本身是反模式。

### 问题 2：`create_pte` 在 "同 PA + 不同 flags" 时的语义与 `identity_map_range` 的预期矛盾

当前 `create_pte(va, pa, flags)` 行为：

```rust
if current.is_valid() {
    if current.paddr() != pa {
        panic!(...);  // 不同 PA → 内核 bug
    }
    // 同 PA → 默默更新 flags（无论是否冲突）
    if current.flags() != leaf_flags {
        table.write(idx, PageTableEntry::new(pa, leaf_flags));
    }
    return Ok(());
}
```

但 `tests/paging-test/src/conflict_panic.rs` 期望：
```rust
pt.create_pte(va, pa, kernel_ro)?;             // 先占位
pt.identity_map_range(..., kernel_rw);             // 以不同 flags 覆盖 → 测试期待 panic
```

行为不一致：
- `create_pte` 文档说"按需更新 flags"（允许冲突）
- `identity_map_range` 测试期望"flags 冲突 panic"
- 现行实现不 panic——测试 `should_panic` 期望落空

这是 **`create_pte` 承担了两种本应分开的语义**：
1. **首次建立**（VA→PA+flags 作为完整契约）
2. **隐式更新**（当 VA 已有 PA 但 flags 不同时，改 flags）

这两种语义应由不同的 API 承担：前者是 `create_pte`（"建立"），后者是 `update_pte`（"仅改 flags"）——两者已经各自存在，但 `create_pte` 错误地兼职了后者。

### 问题 3：`MmioRegion::map` 接收裸 paddr 缺乏 RAM 交叉校验

若驱动（或 FDT 解析错误）将落在 `MEMORY_INFO.physical_memory` 范围内的 paddr 传入，当前会：
1. `create_pte` 在 SAS 背景层对该 PA 的 PTE 把 flags 从 `kernel_rw` 更新为 `kernel_device()`
2. AArch64 下 RAM 被重映射为 Device-nGnRnE——uncacheable + strong ordering，性能骤降 + 潜在一致性问题
3. RISC-V 下（无 Svpbmt）缓存属性无变化，但仍是概念错误

## 量化

| 删除项 | 行数 |
|-------|------|
| `MMIO_REGIONS` 静态 + `SpinLockIrq` 包装 | 5 |
| `check_mmio_overlap` 函数 | 20 |
| `MemoryError::{MmioIdentical, MmioOverlap}` 变体 | 2 |
| `map_mmio` 调用 `check_mmio_overlap` 的匹配块 | 5 |
| **合计** | **~32 行** |

新增 `create_pte` 返回 `FlagsConflict` 分支：~6 行。净减 ~26 行。

## 备选方案

### 方案 A: `create_pte` 新增 `Err(FlagsConflict)` + 删除 MMIO_REGIONS + paddr RAM 校验

**设计**：

```rust
// crates/paging/src/error.rs
pub enum PagingError {
    AllocationFailed,
    PageNotMapped,
    FlagsConflict,   // ← 新增
}

// crates/paging/src/table.rs
pub fn create_pte(&mut self, va, pa, flags) -> Result<(), PagingError> {
    let leaf_flags = flags.for_leaf_at_level(0);
    let (frame_paddr, idx) = self.walk_create(va)?;
    let mut table = unsafe { Table::from_paddr(frame_paddr) };
    let current = table.read(idx);
    if current.is_valid() {
        if current.paddr() != pa {
            panic!(...);  // 不同 PA 仍 panic（内核 bug）
        }
        if current.flags() != leaf_flags {
            return Err(PagingError::FlagsConflict);  // ← 显式拒绝，不再默默更新
        }
        return Ok(());  // 幂等
    }
    table.write(idx, PageTableEntry::new(pa, leaf_flags));
    Ok(())
}

// identity_map_range 遇到 FlagsConflict 直接 panic（boot/MMIO 配置 bug）
```

```rust
// crates/memory/src/lib.rs
pub fn map_mmio(paddr: PhysAddr, size: usize) -> Result<VirtAddr, MemoryError> {
    // paddr RAM 校验
    let info = MEMORY_INFO.get().expect("MEMORY_INFO not initialized");
    let ram_start = info.physical_memory_addr;
    let ram_end = ram_start + info.physical_memory_size;
    assert!(
        paddr.as_usize() + size <= ram_start.as_usize() || paddr >= ram_end,
        "map_mmio: paddr {paddr} + {size:#x} 落在 RAM 范围 [{ram_start}, {ram_end})——拒绝映射为 Device 内存"
    );

    paging::mmio::MmioRegion::map(paddr, size)?;
    Ok(paddr.to_virt())
}

// MMIO_REGIONS / check_mmio_overlap / MmioIdentical / MmioOverlap 全部删除
```

**优点**:
- 单一真相源——PageTable 中的 PTE 本身是 overlap 检测的 canonical state，无需另设 BTreeMap
- `create_pte` 语义清晰：建立契约，flags 冲突告诉调用方（返回 Err，不默默变更）；`update_pte` 仍用于显式改 flags 场景（`OwnedPages::set_flags`、`claim_pages` 等）
- 消除 TOCTOU——`identity_map_range` 遇到冲突时 PTE **未被修改**（`create_pte` 在冲突时不 write），panic 前状态一致
- 消除 `UNSPECIFIED` 锁级别语义错误（静态连同锁一起删）
- paddr RAM 校验在唯一合适的入口（`memory::map_mmio`）防 Device-memory 重映射 RAM
- 修复 `conflict_panic.rs` 测试失败状态

**缺点**:
- `create_pte` 行为变化——当前宽松（默默更新），新版严格（冲突返回 Err）。调用方需要适配：
  - `identity_map_range` 需要 panic 处理新错误——这正是期望行为
  - `OwnedPages::new`、`OwnedPages::set_flags` 通过 `update_pte` 调用路径（不经过 `create_pte`）——不受影响
- `MmioRegion::map` 的 PL011/PLIC 驱动直连调用路径不再经过 `memory::map_mmio` 校验——这些是内核内部驱动，trusted；是否需要将 RAM 校验下沉到 `MmioRegion::map` 自身是次要决策

### 方案 B: 保留 MMIO_REGIONS，修复操作顺序（先检查后映射）

**优点**:
- 不改 `create_pte` 语义

**缺点**:
- BTreeMap 仍重复记录 PTE 已有信息
- 现有 `create_pte` / `identity_map_range` 间的语义矛盾未解决，`conflict_panic.rs` 仍然期望错误的行为
- 锁级别 UNSPECIFIED 问题未修——MMIO_REGIONS + KERNEL_PT 潜在锁序风险仍在

### 方案 C: 保持现状

维持"先映射后检查"、宽松 `create_pte`、`conflict_panic.rs` 测试与代码矛盾。

**优点**: 零改动

**缺点**: 三项问题全部保留

## 决策

选择 **方案 A**（即审查讨论中的 **N2a + H5 + B2** 组合）。

## 理由

- **PageTable 的 PTE 是 MMIO overlap 的自然真相源**——软件层的 BTreeMap 是重复簿记；删除它去 TOCTOU、去锁序问题、去 UNSPECIFIED 锁级别问题、去 `MmioIdentical`/`MmioOverlap` 两个 error 变体
- **`create_pte` vs `update_pte` 是显式意图区分**：`HashMap::insert` vs `HashMap::get_mut().set()` 的类比——建立新键时重复键给警告，已知键的修改走 get_mut；两种意图不应被一个 API 兼职
- **paddr RAM 校验成本极低**（一次算术比较），收益是拒绝一类隐式 RAM 损坏
- **`conflict_panic.rs` 测试的原作者意图是"flags 冲突 = 配置错误"**——方案 A 让代码与意图对齐；方案 C 让测试失败

**Rust 范式层面**：这是对 "接口语义应单一（Single Responsibility Principle 在 API 设计中的体现）" 的应用。一个返回 `Result<(), PagingError>` 的方法不应在成功分支里悄悄改了状态——状态变更应当对调用方可见（返回值、参数、类型变更），否则调用方无法推理。

## 影响

### 代码变更

| 文件 | 变更 |
|------|------|
| `crates/paging/src/error.rs` | `PagingError` 新增 `FlagsConflict` 变体 + `Display` 描述 |
| `crates/paging/src/table.rs` | `create_pte` 在 "同 PA + 不同 flags" 时返回 `Err(FlagsConflict)` 而非默默更新；注释更新"按需更新 flags"→"flags 冲突返回错误" |
| `crates/paging/src/table.rs` | `identity_map_range` 的 match 分支覆盖 `FlagsConflict` → panic |
| `crates/memory/src/lib.rs` | 删除 `MMIO_REGIONS` 静态、`check_mmio_overlap` 函数；`map_mmio` 加 paddr RAM 校验并简化为 "map + 返回 paddr.to_virt()"；删除 `use alloc::collections::BTreeMap` / `SpinLockIrq` 相关 import |
| `crates/memory/src/error.rs` | 删除 `MemoryError::{MmioIdentical, MmioOverlap}` 变体；`From<PagingError>` 新增 `FlagsConflict` 映射为 `MapFailed`（或保留 PagingError 作为 source） |
| `tests/paging-test/src/conflict_panic.rs` | 保持 `should_panic` 属性——代码改动后此测试应 pass（`identity_map_range` 见 `FlagsConflict` 即 panic） |

### API 变更

| 项 | 变更 |
|----|------|
| `PagingError::FlagsConflict` | 新增 |
| `MemoryError::MmioIdentical` | 删除 |
| `MemoryError::MmioOverlap` | 删除 |
| `MemoryError::MapFailed` | 保留（或新增为 FlagsConflict 的映射目标——待实施时权衡） |
| `memory::map_mmio` | 签名不变；行为：部分重叠不再 panic，返回 `Err(MapFailed)` 或类似 |
| `PageTable::create_pte` | 签名不变；新错误分支 `FlagsConflict` |

### 测试

- `tests/paging-test/src/conflict_panic.rs` 无需修改代码——实施 ADR 后应从"测试失败"变为"测试通过"（`identity_map_range` 遇冲突 panic）
- `tests/paging-test/src/table.rs` 的 `test_create_pte_idempotent` 和 `test_create_pte_updates_flags_same_pa`：
  - `test_create_pte_idempotent` 传相同 flags——保持 Ok
  - `test_create_pte_updates_flags_same_pa` 传不同 flags——**需要适配**：改为预期 `Err(FlagsConflict)`；或改测 `update_pte` 显式更新路径
- 新增测试：`test_map_mmio_rejects_ram_address`——传入 RAM 范围内 paddr 应 panic

### 文档

- `crates/paging/src/table.rs::create_pte` doc comment：
  ```
  /// SAS 全量映射下 PTE 始终存在。此方法的语义：
  /// - 若该 VA 无 PTE → 创建 Level 0 叶 PTE，返回 Ok
  /// - 若该 VA 已有 PTE 且 PA 相同且 flags 相同 → 幂等，返回 Ok
  /// - 若该 VA 已有 PTE 且 PA 相同但 flags 不同 → Err(FlagsConflict)；
  ///   显式修改 flags 请使用 update_pte
  /// - 若该 VA 已有 PTE 但 PA 不同 → panic（内核 bug）
  ```
- `crates/memory/src/lib.rs` 模块注释：删除 `MMIO_REGIONS` 和"检测重叠"描述
- `docs/design/memory-subsystem-v2.md` §11.2 `map_mmio` 流程描述：删除 3 步"check_mmio_overlap / MmioIdentical / MmioOverlap" 记述，改为"单步 identity_map_range，冲突由 PageTable 检测并 panic"

## 参考

- [ADR-007](007-eliminate-vma-and-dead-code.md) — 引入 MMIO_REGIONS 的来源（本 ADR 反转其 30 行 MMIO 跟踪决策）
- [Linux `drivers/base/platform.c::platform_device_add_resources`] — Linux 用 `request_resource` 做 MMIO 冲突检测，但其 scheme 与 SimpleKernel 的 PageTable 承担不同职责；SimpleKernel 的 PageTable 已经是真相源
- Rust API Guidelines §C-ERRORS — 失败路径应显式返回，不默默变更状态
