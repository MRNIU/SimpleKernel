<!-- Copyright The SimpleKernel Contributors -->

# ADR-013: 删除 `OwnedPages` 抽象层

> **状态**: 已接受
>
> **日期**: 2026-04-18
>
> **审计阶段**: R3 — 内存子系统（回看）
>
> **涉及模块**: `crates/paging`, `crates/memory`, `crates/config`, `tests/paging-test`

## 背景

`OwnedPages` 在 [ADR-005](005-sas-full-mapping-owned-pages.md) 中从 Theseus `MappedPages` 演进而来，其后由 [ADR-006](006-memory-subsystem-simplification.md)、[ADR-007](007-eliminate-vma-and-dead-code.md)、[ADR-009](009-remove-claimed-bit-and-page-poison.md) 逐步简化：

| 阶段 | `OwnedPages` 形态 |
|-----|------------------|
| ADR-005（2026-04-09）| `ManuallyDrop<MappedFrames>` + `map`/`unmap`/`mprotect`，Drop 恢复默认权限并归还帧；伴随 4-state typestate |
| ADR-006（2026-04-11）| `AllocatedFrames` 直接持有（零 unsafe）；`new`/`set_flags`；删除 `unmap`；2-state |
| ADR-007（2026-04-12）| 消除 VMA，`OwnedPages` 的"VMA 簿记"消费者消失，剩下的消费者以 `mem::forget` 永久持有 |
| ADR-009（2026-04-17）| 删除 CLAIMED 软件位，Drop 仅保留"恢复 `kernel_rw` + 写 poison + 归还帧" |
| 当前 | ~50 行 struct + Drop；1 个生产调用点；1 份测试套件 |

核心概念定位（见 `docs/design/memory-subsystem-v2.md` §6）是**权限守卫**：与 `MutexGuard` 同构的 RAII，`new` 获取帧 + 设权限，`Drop` 恢复 `kernel_rw` + 归还帧。

## 当前消费现状

对整个工作区扫描 `OwnedPages` 使用点：

| 调用点 | 目的 | Drop 是否执行 |
|-------|------|-------------|
| `crates/memory/src/init.rs:79` | 为 `.text`/`.rodata`/`.data` 覆盖权限 | 否——`mem::forget` |
| `tests/paging-test/src/mapping.rs` | 6 个测试（new / multi-page / preexisting / changes_flags / drop / set_flags）| 部分测试测 Drop 行为 |

对照 SAS 架构中其他永久性帧持有路径，都**绕过** `OwnedPages`：

| 持有者 | 方式 |
|-------|------|
| 堆扩展帧（`memory::init`）| `AllocatedFrames::alloc` + `mem::forget` |
| 页表节点帧（`PageTable::nodes`）| `Vec<AllocatedFrames>`，随页表生命周期 |
| MMIO 映射（`MmioRegion`）| 平级类型，从不持有帧（非 RAM），无 Drop 逻辑 |
| 未来 DMA / 动态堆区 | 尚未实现 |

观察要点：

1. **Drop 分支在生产中从未执行**。`batch_update_flags → kernel_rw`、`write_bytes(FREED_PAGE_POISON)`、`AllocatedFrames` 归还 buddy——这三步逻辑仅在单元测试里触发。
2. **`set_flags` 零生产调用**。
3. **SAS 架构的实际内存模型**是"全部永久持有 + 权限设定后不变"。没有短生命周期的、需要回收权限并归还帧的场景。

## ADR-008 的论据同构性

[ADR-008](008-eliminate-frame-state-typestate.md) 删除 `FrameState` typestate 时，论据结构为：

> typestate 的经典价值是"在编译期阻止状态误用"，但本 crate 的实际 API 面上……
> `FrameState` 编码的状态机在当前 API 边界上**无任何用户可见面**……
> "能在编译期做的就在编译期做" ≠ "凡是能放到编译期的就应该放"

对 `OwnedPages` 的 RAII 语义套用同一审视：

- **RAII 的经典价值**：构造=获取资源，析构=释放资源，编译期保证配对
- **当前 API 面**：唯一生产路径是 `new(...) → mem::forget`，释放端从未触达
- **SAS 模型下的资源归属**：帧/权限归属全部是"boot 建立 → 永久持有"

这一致性提示：与 `FrameState` 一样，`OwnedPages` 的 RAII 机制在当前 API 边界上**没有被消费的那一半**。

## 备选方案

### 方案 A：保留现状

维持 `OwnedPages` 结构与 `new`/`set_flags`/`Drop` 语义。

**优点**：
- 零改动；设计文档、README、测试全部对齐
- 概念清晰：与 Theseus `MappedPages`、`MutexGuard` 同构，学习成本低
- 未来引入 DMA buffer、可回收内核内存区、模块热加载等**短生命周期帧 + 权限覆盖**场景时直接可用
- 保留"帧所有权与权限语义打包"的类型，有助于表达设计意图

**缺点**：
- 违反 CLAUDE.md 原则 "Don't design for hypothetical future requirements"
- 与 ADR-008 删除 `FrameState` 的理由自相矛盾——两者都是"编译期保证没有用户可见面的消费者"
- Drop 分支（~20 行含 poison）属于死代码，无法通过生产测试覆盖
- 测试套件自证——既然唯一生产用法是 `new + forget`，那"Drop 恢复权限"这条测试断言保护的行为在实际产品中不发生
- `set_flags` API 零消费者

### 方案 B：删除 `OwnedPages`，权限设定降为 `paging` 自由函数

`paging` 暴露一个 `apply_flags(frames: &AllocatedFrames, flags: PteFlags)` 或 `set_range_flags(va: VirtAddr, count: usize, flags: PteFlags)` 自由函数，内部调用 `batch_update_flags`。`init.rs` 显式 `mem::forget(frames)`，不再经过守卫类型。

```rust
// memory/src/init.rs 改写后
for ((start, flags), frames) in segments.into_iter().zip(reserved) {
    paging::set_range_flags(start.to_virt(), frames.page_count(), flags);
    core::mem::forget(frames);
}
```

**优点**：
- 删除 `crates/paging/src/mapping.rs`（~130 行）和 `tests/paging-test/src/mapping.rs` 中 OwnedPages 特有测试
- 与 ADR-008（删除 `FrameState`）论据对称：没有消费者的编译期保证被下调
- `init.rs` 更直白——"改权限 + 放弃帧"两步语义显式
- `MmioRegion` 单独作为唯一"类型化映射"，而非两个平级概念并存
- 消除 `FREED_PAGE_POISON` 相关配置（若同时决定不再保留 poison）

**缺点**：
- 破坏性 API 变更：`paging::OwnedPages` 公开符号消失
- 未来引入真正需要 Drop 的帧+权限打包场景（DMA、可回收内核段）需要重新设计
- 失去"帧所有权与权限作为单一资源"的类型化表达——调用方需分别管理两者
- 设计文档 `memory-subsystem-v2.md` §6 需要重写
- 可能需要判断是否同时删除 `FREED_PAGE_POISON` poison 填充机制（若保留 poison，需要让自由函数接一个独立的 `release` 入口——复杂度部分回流）

### 方案 C：保留结构，删除 Drop 与 `set_flags`

`OwnedPages` 降级为"帧 + 权限"的打包视图，无 Drop 行为（或仅归还帧），不提供权限修改接口。

```rust
pub struct OwnedPages {
    frames: AllocatedFrames,
    flags: PteFlags,
}
impl OwnedPages {
    pub fn new(frames: AllocatedFrames, flags: PteFlags) -> Self { ... }
    pub fn vaddr(&self) -> VirtAddr { ... }
    pub fn page_count(&self) -> usize { ... }
    pub fn flags(&self) -> PteFlags { ... }
}
// 无 Drop，或仅实现 AllocatedFrames 的自然 Drop（归还 buddy）
```

**优点**：
- 保留"帧 + 权限"打包的类型化表达
- 消除死代码：poison 填充、kernel_rw 恢复、TLB flush
- 比方案 B 改动更小；`init.rs` 仍可写 `OwnedPages::new(...) + mem::forget`
- 如果未来需要 Drop 行为，可以基于此类型扩展

**缺点**：
- 半成品——保留 struct 但砍掉 Drop，类型命名（"Owned"）与行为（无生命周期管理）语义错位，可能比方案 B 更令人困惑
- 保留了与 Theseus `MappedPages` 的视觉相似性但丢了实质
- 违反 "RAII 类型应当通过 Drop 传达所有权语义" 的 Rust 惯例

## 决策

选择 **方案 B**——删除 `OwnedPages`，权限设定降为 `PageTable::update_range_flags` 方法。

## 理由

### 关键证据：潜在消费者经核实均不复用 OwnedPages

初稿列出的两个"未来可能"场景（DMA buffer 与用户程序）经代码和历史 P9/BusyBox 草案结论验证后，**均不会消费 `OwnedPages`**。相关草案结论已经吸收到本 ADR；生成工具留下的历史计划不再作为长期真值源保留在仓库中。

**DMA buffer**（`src/device/hal.rs`）：

- `virtio_drivers::Hal::dma_alloc` trait 签名是 `(u64, NonNull<u8>)`——**trait 边界强制裸指针**，move-only 类型传不过去
- 当前实现已用 `SpinLock<BTreeMap<u64, AllocatedFrames>>` 追踪帧生命周期，不经过 `OwnedPages`
- DMA 真正特有需求（cache flush / invalidate、类型化 `DmaBuffer<T>`、IOMMU hook）与 `OwnedPages` 的权限覆盖 + poison 语义**特性集合几乎不重叠**
- 结论：DMA 若需要类型化抽象，应设计专用 `DmaBuffer<T>`，不复用 `OwnedPages`

**用户程序加载**：

历史 P9/BusyBox 草案中的 "混合模型" 决策是：内核侧保持 SAS，用户侧走传统 U-mode + 独立页表。P9 的 `UserVma` 草案直接持有 `Vec<AllocatedFrames>`，**明确绕过 `OwnedPages`**。根本原因：

- **VA ≠ PA**：用户进程有独立页表，`OwnedPages::vaddr() = PA.to_virt()` 的 identity-mapping 假设不成立
- **创建 PTE vs 覆盖 flags**：SAS 下 `OwnedPages::new` 是在背景 PTE 上改 flags；用户侧是从无到有建 PTE（`map_page`），非同一原语
- **Drop 语义不同**：用户 VMA 的 Drop 需要**清除 PTE**（ADR-007 已删除的 `unmap_page`）；`OwnedPages::Drop` 只恢复 flags
- **所有权粒度不同**：用户侧粒度是"整个 `UserAddressSpace`"，不是"一个帧区间"

**其他潜在消费者**：

| 场景 | 现状 |
|-----|------|
| 任务内核栈（`src/task/tcb.rs:21`）| `Vec<u8>` 堆分配，不涉及帧级权限 |
| 堆扩展 | `AllocatedFrames::alloc + mem::forget`，绕过 OwnedPages |
| 页表节点 | `PageTable::nodes: SpinLock<Vec<AllocatedFrames>>`，绕过 OwnedPages |
| MMIO | `MmioRegion` 平级类型，从不持有 RAM 帧 |

剩下理论上可能需要 `OwnedPages` 的只有 Theseus 风格的**内核模块热加载**，但该方向在当前路线图（P8–P12 BusyBox）中无规划。

### Rust 范式层面

- **RAII 的价值在于成对**：构造获取资源 + 析构归还资源。当析构端永远被 `mem::forget` 跳过，RAII 类型承诺只剩前半段。Rust 惯例是永久资源直接 `mem::forget`（如 `Box::leak`、`Arc::into_raw`），不需要专门包一层类型
- **SAS 全量映射下"设置权限"是幂等操作**：不是对"帧 + 权限"复合资源的生命周期事件。用自由方法表达幂等操作、用 `AllocatedFrames` 单独表达帧所有权、用 `mem::forget` 显式表达永久持有——三个概念各司其职，比"权限守卫"打包语义更贴近实际

### 与 ADR-008 的一致性

ADR-008 删除 `FrameState` typestate 的核心论据是 "当前 API 边界上无用户可见面"。对 `OwnedPages` 套用同一标准：

| 机制 | 用户可见面 | 结论 |
|-----|-----------|------|
| `FrameState` typestate | `Frames<Free>` 不 pub，转换发生在 3 行代码内 | ADR-008：删除 |
| `OwnedPages::Drop` | 生产中从未触发 | 本 ADR：删除 |
| `OwnedPages::set_flags` | 零调用者 | 本 ADR：删除 |

两者论据同构：编译期机制保护的不变量没有需要保护的消费者。

### 方案 A/C 不选的理由

- **方案 A（保留）**：违反 CLAUDE.md "Don't design for hypothetical future requirements"；与 ADR-008 论据自相矛盾；Drop 分支是死代码无法通过生产测试覆盖
- **方案 C（降级为视图类型）**：类型名"Owned"保留但 Drop 行为砍掉，语义错位更令人困惑；在 Rust 惯例（RAII 类型应通过 Drop 传达所有权语义）面前是半成品

### 关于 `FREED_PAGE_POISON`

`config::FREED_PAGE_POISON`（ADR-009 保留）的**唯一消费者是 `OwnedPages::Drop`**。删除 `OwnedPages` 后 poison 填充完全没有触发路径，成为孤立常量。

本 ADR 的处理方式：同步从 `config` 删除 `FREED_PAGE_POISON` 常量。ADR-009 对 poison 的"纵深防御"论述依赖"Drop 时填充"这一机制——机制消失后，保留常量本身无意义。若未来引入带 Drop 的专用类型（`DmaBuffer<T>` 等）认为需要 poison，届时重新引入即可。

### 测试自证问题

`tests/paging-test/src/mapping.rs` 中 `test_drop_restores_default_flags` 与 `test_set_flags_changes_flags` 测试的行为在当前生产中不发生。这类测试本身不是错误（可以防止回归），但它们保护的不变量没有生产消费者。本 ADR 整体删除这两个测试 + `test_new_*` 的大部分场景（因为 `identity_map_range` / `update_pte` 已有测试覆盖等价行为）。

## 影响

- **代码变更**：
  - 删除 `crates/paging/src/mapping.rs`（~130 行）
  - `crates/paging/src/table.rs` 新增 `PageTable::update_range_flags(va_start, page_count, flags)` 方法（~10 行，封装原 `batch_update_flags` 逻辑）
  - `crates/paging/src/lib.rs` 删除 `pub mod mapping` 与 `pub use mapping::OwnedPages`
  - `crates/memory/src/init.rs` 改为 `kernel_page_table().update_range_flags(...)` + `mem::forget(frames)`
  - `crates/config/src/lib.rs` 删除 `FREED_PAGE_POISON` 常量（孤立）
- **API 变更**：
  - `paging::OwnedPages` 公开符号消失
  - `paging::kernel_page_table().update_range_flags()` 新增
- **测试变更**：
  - 删除 `tests/paging-test/src/mapping.rs`（6 个测试）
  - `tests/paging-test/Cargo.toml` 中的 `[[bin]] mapping` 条目删除
  - 为 `update_range_flags` 在现有 `tests/paging-test/src/table.rs` 中补一份最小测试
- **文档变更**：
  - `docs/design/memory-subsystem-v2.md` §6（权限覆盖）重写——不再有"权限守卫"类型，改为描述 `update_range_flags` 方法
  - `crates/paging/README.md` 删除 `OwnedPages` 条目
  - `crates/frame_allocator/README.md` 删除 `OwnedPages` 配合示例
  - `crates/memory/AGENTS.md` 示例改写为 `update_range_flags + mem::forget`
  - `crates/page_table_entry/src/lib.rs` 删除引用 `OwnedPages::set_flags` 的注释

## 参考

- [ADR-005: SAS 全量映射 + OwnedPages 所有权模型](005-sas-full-mapping-owned-pages.md) — 引入 OwnedPages 的决策
- [ADR-006: 内存子系统简化——4KB 单页 + 2-state + 权限覆盖模型](006-memory-subsystem-simplification.md) — 从 `ManuallyDrop` 简化为直接持有 `AllocatedFrames`
- [ADR-007: 消除 VMA 模块及内存子系统死代码](007-eliminate-vma-and-dead-code.md) — VMA 删除后 `OwnedPages` 剩下唯一 `mem::forget` 消费者
- [ADR-008: 删除 FrameState typestate](008-eliminate-frame-state-typestate.md) — 论据结构同构
- [ADR-009: 删除 CLAIMED 软件位，保留 page poison](009-remove-claimed-bit-and-page-poison.md) — poison 填充当前仅在 OwnedPages::Drop 中触发，若选方案 B 需同步评估
- [Theseus OSDI'20 §4.3 `MappedPages`](https://www.usenix.org/system/files/osdi20-boos.pdf) — 原始 RAII 设计，但 Theseus 场景有大量短生命周期映射
- `docs/design/SAS-架构设计.md` §1.2 — "MappedPages 仿射类型防止非法映射操作"的原始论述
- `docs/design/memory-subsystem-v2.md` §6 — "权限守卫"的现行概念模型
- CLAUDE.md — "Don't design for hypothetical future requirements"、"Don't add features, refactor, or introduce abstractions beyond what the task requires"
