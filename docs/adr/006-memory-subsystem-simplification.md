<!-- Copyright The SimpleKernel Contributors -->

# ADR-006: 内存子系统简化——4KB 单页 + 2-state + 权限覆盖模型

> **状态**: 已接受
>
> **日期**: 2026-04-11
>
> **审计阶段**: R3 — 内存子系统
>
> **涉及模块**: `crates/frame_allocator`, `crates/paging`, `crates/memory`, `crates/memory_types`, `crates/page_table_entry`
>
> **取代**: [ADR-005](005-sas-full-mapping-owned-pages.md)（保留并细化其核心决策"全量映射 + OwnedPages"，简化实现）

## 背景

SimpleKernel 内存子系统经历了多轮演进：

- [ADR-003](003-sas-identity-mapping-only.md) 移除了 `page_allocator`，确立 identity mapping only
- [ADR-005](005-sas-full-mapping-owned-pages.md) 确立了全量映射 + `OwnedPages` 所有权模型

ADR-005 的核心决策——**全量映射 + typestate 追踪所有权而非 PTE 生命周期**——是正确的。
但当前代码仍保留以下来自 Theseus 的设计遗留，与新模型矛盾：

### 问题 1：4-state typestate 不追踪任何硬件状态变化

`frame_allocator` 有 Free/Allocated/Mapped/Unmapped 四个状态。
在 Theseus 中，这四个状态追踪真实的 PTE 生命周期：

- `Allocated → Mapped`：帧地址写入 PTE + EXCLUSIVE 位置 1
- `Mapped → Unmapped`：PTE 清零 + 从 PTE 恢复帧地址

但在 SimpleKernel 全量映射下，PTE 始终存在（背景 identity mapping）。
`Mapped`/`Unmapped` 状态不对应任何硬件状态变化——帧从不"沉入 PTE"，
始终在 Rust struct 字段中。这两个状态只是类型标签的变更。

`MappedFrames::Drop` 的 panic 保护了一个不存在的场景
（"帧沉入 PTE 但类型被丢了"）。

### 问题 2：map/unmap 术语与实际操作不符

`OwnedPages::map`/`unmap`/`mprotect` 暗示"创建/删除映射"。
实际操作是"改 PTE 权限/恢复 PTE 权限"——映射始终存在，从不创建或删除。

### 问题 3：ManuallyDrop + unsafe

因为 `MappedFrames::Drop` 会 panic，`OwnedPages` 用 `ManuallyDrop` 包装帧，
引入 2 处 `unsafe { ManuallyDrop::take() }`。
这些 unsafe 完全是 4-state 设计导致的——如果帧直接存为 `AllocatedFrames`，
`OwnedPages::Drop` 恢复权限后帧自动归还，零 unsafe。

### 问题 4：大页支持增加复杂性但无可观测收益

`identity_map_range` 支持 2MB/1GB 大页映射。但在 SAS + QEMU 下：

- 页表内存差异：128MB 内存，全 4KB 约 264KB，大页约 12KB——差 252KB（0.2%）
- TLB 性能差异：QEMU 不模拟 TLB；SAS 不刷 TLB（无上下文切换刷新）
- 大页引入的复杂性：page splitting（改大页中部分 4KB 页的权限时需要拆分大页）、
  多级 map/unmap、`PageSize` 泛型参数贯穿 `Frames<S, P>`

大页真正重要的场景（数据库大内存、多进程 TLB flush 风暴、百 GB 级映射）
在 SimpleKernel 的 SAS + QEMU + 128MB 环境下均不存在。

### 与 Theseus 的根本差异

这些遗留问题的根源是 SimpleKernel 与 Theseus 的内存模型已经**根本不同**：

| | Theseus | SimpleKernel |
|--|---------|-------------|
| 页表的角色 | 隔离机制——PTE 有无决定帧能否访问 | 权限管控（纵深防御）——PTE 始终存在 |
| "映射"的含义 | 创建 VA→PA 翻译 | 不存在——所有翻译永远存在 |
| 帧所有权追踪 | PTE EXCLUSIVE 位（运行时硬件位） | Rust struct 字段（编译期） |
| 安全边界 | PTE 有/无 = 可/不可访问 | PTE 权限位 = 可/不可做某操作 |

代码应该完全反映这个差异，不再保留 Theseus 的术语和状态机。

### 学术定位

SimpleKernel 处于以下研究线的交汇点：

- 全量映射：Linux direct map、rCore identity map
- 语言级隔离：Theseus OSDI'20、RedLeaf OSDI'20、SPIN SOSP'95
- 权限覆盖模型——硬件类比：Intel MPK、ARM POE（映射不变，只改权限）
- 纵深防御：语言安全为主 + 硬件权限为辅（libhermitMPK VEE'20、SafeBPF CCSW'24）

参见 `docs/design/references.md` §6。

## 备选方案

### 方案 A: 全面简化

1. `frame_allocator`: 4-state → 2-state（Free/Allocated），枚举 `MemoryState` → `FrameState`
2. `OwnedPages`: `map` → `new`，`mprotect` → `set_flags`，删除 `unmap`；消除 ManuallyDrop + unsafe
3. 去掉大页映射支持（4KB-only），删除 `PageSize` 泛型
4. init 顺序调整为"先背景映射全部物理内存，再 OwnedPages 覆盖内核段权限"
5. 页表方法语义调整（`map_page` → `set_page_flags` 等）

**优点**:
- 消除所有 Theseus 遗留的概念错配
- `OwnedPages` 零 unsafe
- 去掉大页消除 page splitting、多级映射的复杂性
- 命名与实际语义一致（"设权限"而非"建映射"）
- 设计文档、代码、概念模型三者统一

**缺点**:
- 重构范围中等（frame_allocator + paging + memory + tests）
- 去掉大页后，真实硬件上 TLB 效率可能有微小影响（SAS 下可忽略）
- `PageSize` 泛型删除后，未来如需大页要重新加回

### 方案 B: 仅重命名，保留 4-state 和大页

只改 `OwnedPages` API 名称，不动 `frame_allocator` 和页表。

**优点**:
- 改动最小

**缺点**:
- ManuallyDrop + unsafe 仍在
- 4-state 概念错配未解决
- 大页复杂性仍在

### 方案 C: 保持现状

**优点**:
- 零改动

**缺点**:
- 设计文档（2-state）与代码（4-state）矛盾持续存在
- 命名（map/unmap）与实际语义（改权限/恢复权限）不一致

## 决策

选择 **方案 A**——全面简化。

## 理由

- **方案 B 治标不治本**：重命名不解决 ManuallyDrop/unsafe、4-state 概念错配
- **方案 C 维持矛盾**：设计文档已描述 2-state，代码是 4-state，继续分裂
- **方案 A 的前提条件均已满足**：
  - SAS 架构不变（ADR-005 决策）
  - 全量映射不变（ADR-005 决策）
  - 两目标架构均使用 4KB 基础页（config `PAGE_SIZE = 4096`）
  - 大页在 SAS + QEMU 下无可观测收益
  - AArch64 如切换 granule（16KB/64KB），需重新设计整个页表子系统——保留大页代码不能避免这个成本

## 影响

### 1. frame_allocator crate

| 文件 | 变更 |
|------|------|
| `state.rs` | `MemoryState` → `FrameState`，删除 `Mapped`/`Unmapped` 变体 |
| `state.rs` | 删除 `MappedFrames`/`UnmappedFrames` 类型别名 |
| `state.rs` | Drop 简化——删除 Mapped panic 分支，统一归还 buddy |
| `state.rs` | `Frames<S, P>` → `Frames<S>`，删除 `P: PageSize` 泛型 |
| `transitions.rs` | 删除 `into_mapped()`/`into_unmapped()`/`into_allocated()`/`into_free()` |
| `alloc.rs` | 删除 `P::NUM_4K_PAGES_SHIFT` 换算，`alloc()` 只接受 4KB 帧数 |
| `lib.rs` | 更新 pub use |

### 2. paging crate

| 文件 | 变更 |
|------|------|
| `mapping.rs` | `OwnedPages.frames`: `ManuallyDrop<MappedFrames>` → `AllocatedFrames` |
| `mapping.rs` | `map()` → `new()`，`mprotect()` → `set_flags()`，删除 `unmap()` |
| `mapping.rs` | 删除 `pub fn frames()` 访问器 |
| `mapping.rs` | Drop 简化——零 unsafe，直接 `restore_default_flags` + 自动 Drop 帧 |
| `table.rs` | `map_page` → `set_page_flags`（语义：存在则更新 flags，不存在则创建） |
| `table.rs` | `map_at_level` 删除（合并入 `set_page_flags`，固定 level 0） |
| `table.rs` | `identity_map_range` 简化为 4KB-only 循环调用 `set_page_flags` |
| `table.rs` | `unmap_at_level_with_flags` 简化（固定 level 0） |

### 3. memory crate

| 文件 | 变更 |
|------|------|
| `init.rs` | init 顺序调整——先背景映射全部物理内存（kernel_rw），再 OwnedPages 覆盖内核段权限 |
| `vma.rs` | `OwnedPages::map` → `OwnedPages::new` |
| `lib.rs` | 更新 re-export |

### 4. 其他 crate

| crate | 变更 |
|-------|------|
| `memory_types` | 删除 `Page2M`/`Page1G` 类型；简化或删除 `PageSize` trait（只剩 `Page4K`） |
| `page_table_entry` | `for_leaf_at_level(level)` 简化（固定 level 0） |

### 5. 测试

| 文件 | 变更 |
|------|------|
| `tests/frame-test/src/alloc.rs` | 删除 `into_mapped`/`into_unmapped` 调用 |
| `tests/frame-test/src/mapped_drop_panic.rs` | 整个测试删除（不再有 Mapped Drop panic） |
| `tests/paging-test/src/mapping.rs` | 方法名更新，返回类型更新 |

### 6. API 变更汇总

| 旧 | 新 |
|----|-----|
| `MemoryState` | `FrameState` |
| `MemoryState::Mapped` / `Unmapped` | 删除 |
| `MappedFrames` / `UnmappedFrames` | 删除 |
| `AllocatedFrames<P: PageSize>` | `AllocatedFrames`（无泛型） |
| `Frames<S, P>` | `Frames<S>`（无 `P`） |
| `OwnedPages::map(frames, flags)` | `OwnedPages::new(frames, flags)` |
| `OwnedPages::mprotect(flags)` | `OwnedPages::set_flags(flags)` |
| `OwnedPages::unmap() → UnmappedFrames` | 删除 |
| `OwnedPages::frames() → &MappedFrames` | 删除 |
| `PageTable::map_page(va, pa, flags)` | `PageTable::set_page_flags(va, pa, flags)` |
| `PageTable::map_at_level(va, pa, flags, level)` | 删除 |

### 7. 不变量记录

内核段帧（.text/.rodata/.data）通过 `frame_allocator::init` 的 `reserved` 参数直接构造为
`AllocatedFrames`，**从未进入 buddy**。若意外 Drop，`dealloc_to_backend` 会将未注册的帧交给
buddy 导致状态污染。

**不变量**：内核段的 `AddressSpace` 存于 `spin::Once<SpinLock<AddressSpace>>`，
`'static` 生命周期保证不会 Drop。

### 8. 文档

- `docs/design/memory-subsystem.md`：头部标注"已过时，见 memory-subsystem-v2.md"
- 新建 `docs/design/memory-subsystem-v2.md`：完整描述新设计
- `CLAUDE.md`：更新 CODE MAP 和相关描述

## 参考

- [Theseus OSDI'20](https://www.usenix.org/system/files/osdi20-boos.pdf) — 4-state typestate 的原始设计，SimpleKernel 从此出发并分道扬镳
- [libhermitMPK VEE'20](https://www.ssrg.ece.vt.edu/papers/vee20-mpk.pdf) — Rust unikernel + MPK 权限覆盖，0.6% 开销。SimpleKernel 在软件层面做了类似的事
- [ADR-003](003-sas-identity-mapping-only.md) — 移除 page_allocator（本 ADR 保留其决策）
- [ADR-005](005-sas-full-mapping-owned-pages.md) — 全量映射 + OwnedPages（本 ADR 取代并细化其实现方案）
- `docs/design/references.md` §6 — 硬件权限隔离机制完整参考列表
