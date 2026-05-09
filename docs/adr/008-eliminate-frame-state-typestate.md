<!-- Copyright The SimpleKernel Contributors -->

# ADR-008: 删除 FrameState typestate 与 `adt_const_params` nightly 依赖

> **状态**: 已接受
>
> **日期**: 2026-04-17
>
> **审计阶段**: R3 — 内存子系统（回看）
>
> **涉及模块**: `crates/frame_allocator`

## 背景

[ADR-006](006-memory-subsystem-simplification.md) 将物理帧的 typestate 从 4-state 简化为 2-state（Free/Allocated）。简化是正确方向，但 ADR-006 的备选方案里没有评估 **0-state（纯 struct）** 路线。

ADR-007 删除 VMA 与 `unmap` 后复盘当前实现：

```rust
// crates/frame_allocator/src/state.rs
#![feature(adt_const_params)]

#[derive(PartialEq, Eq, ConstParamTy)]
pub enum FrameState {
    Free,
    Allocated,
}

pub struct Frames<const S: FrameState> { range: FrameSpan }

pub type FreeFrames      = Frames<{ FrameState::Free }>;      // 不 pub export
pub type AllocatedFrames = Frames<{ FrameState::Allocated }>; // 用户可见
```

### 问题 1：typestate 无用户可见面

2-state 现状：

1. `FreeFrames` **不 pub export**——仅作为 crate 内部的 `alloc_from_backend` 返回类型
2. `FreeFrames → AllocatedFrames` 转换只在一个地方发生（`AllocatedFrames::alloc` 内部的 3 行）
3. 两状态的 `Drop` 行为完全一致（都调 `dealloc_to_backend`）
4. 两状态除 Drop 外**没有任何重叠方法**——它们不通过共享 impl 受益于泛型抽象

typestate 的经典价值是 **"在编译期阻止状态误用"**。但本 crate 的实际 API 面：

| 防御场景 | 2-state typestate 提供 | 替代机制（普通 struct） |
|---------|----------------------|---------------------|
| 未分配就使用帧 | `FreeFrames` 不 pub → 外部只能拿 Allocated | `AllocatedFrames::alloc` 是唯一公开构造器 |
| 释放后使用帧 | Drop 消费 self → 编译错误 | `Drop` 消费 self → 编译错误（一样） |
| 内部错把 Free 当 Allocated | 类型不同，函数签名阻止 | 单一 struct，crate 内 3 行代码不会发生 |

换言之，`FrameState` 编码的状态机在当前 API 边界上**无任何用户可见面**，也无必须通过泛型抽象复用的公共逻辑。

此外，`#![feature(adt_const_params)]` 是不稳定 nightly 特性：

- 在 [RFC 2000](https://rust-lang.github.io/rfcs/2000-const-generics.html) / 后续 `adt_const_params` 特性中持续演进
- 每次 Rust nightly toolchain 升级都有潜在破坏风险
- 参见 [tracking issue #95174](https://github.com/rust-lang/rust/issues/95174)

### 问题 2：分配器不应承担清零策略

当前 `AllocatedFrames::alloc` 内部硬编码了 `write_bytes(0, ...)` 清零：

```rust
pub fn alloc(count: usize) -> Result<Self, FrameAllocError> {
    let free = alloc_from_backend(count)?;
    unsafe {
        let ptr = free.start_paddr().to_virt().as_mut_ptr::<u8>();
        core::ptr::write_bytes(ptr, 0, count * PAGE_SIZE);  // ← 强制清零
    }
    Ok(free.into_allocated())
}
```

问题：

1. **职责错位**：分配器的职责是"给帧/收帧"，初始化策略属于调用方决策。Linux 的 `alloc_pages(GFP_KERNEL)` 不清零，`get_zeroed_page` / `GFP_ZERO` 是可选语义——分配器提供机制，调用方选择策略
2. **浪费**：`alloc_node_frame()` 分配页表节点帧——`alloc` 清零一遍，然后 `walk_create` 里又通过 `Table::write` 逐个写入 PTE，第一遍清零多余（虽然页表节点确实需要全零初态，但这应该由页表模块决定，不是分配器强加）
3. **正确的原则**：内核内部不应假设分配出来的内容可靠——所有使用者必须显式初始化，但"显式初始化"不等于"分配器清零"

## 备选方案

### 方案 A: 删除 FrameState + 分配器不清零

```rust
pub struct AllocatedFrames { range: FrameSpan }

impl AllocatedFrames {
    /// 分配 `count` 个连续 4K 帧。**内容未初始化**，调用方负责初始化。
    pub fn alloc(count: usize) -> Result<Self, FrameAllocError> {
        alloc_from_backend(count)    // 直接返回 AllocatedFrames，不清零
    }
    pub fn alloc_one() -> Result<Self, FrameAllocError> { Self::alloc(1) }
    pub fn count(&self) -> usize { self.range.size() }
    pub fn start_paddr(&self) -> PhysAddr { self.range.start().start_addr() }
    pub(crate) fn from_range(range: FrameSpan) -> Self { Self { range } }
}

impl Drop for AllocatedFrames {
    fn drop(&mut self) { dealloc_to_backend(self.range); }
}
```

`alloc_from_backend` 直接返回 `AllocatedFrames`，不清零。各调用方按需初始化：

```rust
// paging: 页表节点帧需要全零（无效 PTE = 0），由 paging 模块自己清零
fn alloc_node_frame() -> Result<AllocatedFrames, PagingError> {
    let frame = AllocatedFrames::alloc_one()?;
    unsafe {
        core::ptr::write_bytes(
            frame.start_paddr().to_virt().as_mut_ptr::<u8>(), 0, PAGE_SIZE,
        );
    }
    Ok(frame)
}

// OwnedPages: 用户帧——看具体场景决定是否清零
// 栈空间：立即写入，可以不清零
// DMA buffer：需要清零，调用方负责
```

**优点**:
- 删除 `state.rs`（85 行）、`transitions.rs`（45 行），整个 `FrameState` 枚举和 const generic 机制
- 移除 `#![feature(adt_const_params)]`——frame_allocator crate 可在稳定 Rust 编译（依然受限于工作区其他 crate 的 nightly 特性，但消除一项）
- API 表面只暴露"已分配帧"这一用户真正需要的概念
- 与 `OwnedPages`（RAII，无 typestate）对称：物理侧/虚拟侧都是"编译期地址类型 + 运行时 RAII 结构"，概念同构
- 分配器不做策略决策——与 Linux `alloc_pages` / Theseus `allocate_frames` 一致
- 消除页表节点帧的冗余清零（当前 `alloc` 清零一遍，`walk_create` 又写入 PTE）

**缺点**:
- 调用方必须自行初始化——忘记初始化则读到旧数据。但在 Rust 中访问未初始化内存需要 `unsafe`，编写者有义务保证初始化
- 未来若 DMA pinned / COW 等状态引入，仍可重新分化——届时基于实际需求设计状态机，而非为未来需求预留 typestate 骨架

### 方案 B: 保持现状

**优点**:
- 零改动
- 如果未来真引入新状态（如 DMA pinned），typestate 骨架复用

**缺点**:
- `adt_const_params` nightly 依赖持续存在
- `FreeFrames` 在 pub 类型（`Frames<const S>`）上留下 "这里可能有其他状态" 的暗示，但实际上没有
- 每次读代码都要理解 2-state 的存在理由，而理由其实没有

### 方案 C: 恢复 4-state（Theseus 风格）

回到 ADR-006 前的 Free/Allocated/Mapped/Unmapped。

**优点**:
- 与 Theseus 一致

**缺点**:
- ADR-006 已经论证过 Mapped/Unmapped 在 SAS 全量映射下不对应任何硬件状态变化
- 完全开倒车

## 决策

选择 **方案 A**。

## 理由

- **方案 B 的"未来可能复用"不构成保留理由**：项目原则"不为假设性需求设计"；未来真需要时引入当时所需的具体类型，而非为猜想预留框架
- **方案 C 违反 ADR-006 已有论证**
- **方案 A 的前提条件均满足**：
  - Rust ownership + Drop 语义提供了 typestate 所声称的所有编译期保证
  - `FreeFrames` 不跨函数、不跨模块、不 pub——删除后无 API 破坏
  - 与 `OwnedPages` 在概念层面对称化
  - 分配器不清零遵循"机制与策略分离"原则

**Rust 范式层面**：这是对 "typestate 不是免费的" 的承认——typestate 的价值在于**在 API 边界上强制状态转换**。如果所有状态转换都发生在单个函数内部、所有用户可见的类型只有一种状态，那么 typestate 是用编译期机制表达运行时不需要表达的东西，是成本大于收益的过度抽象。这与项目"编译期保证优于运行时保证"原则不矛盾——那条原则是说"能在编译期做的就在编译期做"，不是"凡是能放到编译期的就应该放"。

**分配器设计层面**：分配器的职责是资源管理（分配/回收），不是内容策略（清零/填充）。Linux 的 `alloc_pages` 不清零（清零是 `GFP_ZERO` 可选标志），Theseus 的 `allocate_frames` 不清零。内核内部不应假设分配结果的内容可靠——所有调用方必须显式初始化，但初始化的方式和时机由调用方根据场景决定。

## 影响

### 代码变更

| 文件 | 变更 |
|------|------|
| `crates/frame_allocator/src/lib.rs` | 删除 `#![feature(adt_const_params)]`；更新 pub use；删除 `mod state` / `mod transitions` |
| `crates/frame_allocator/src/state.rs` | **整个文件删除** |
| `crates/frame_allocator/src/transitions.rs` | **整个文件删除** |
| `crates/frame_allocator/src/alloc.rs` | `alloc_from_backend` 返回类型改为 `AllocatedFrames`；**删除清零逻辑**；删除 `into_allocated` 调用 |
| `crates/frame_allocator/src/alloc.rs` | 新增 `AllocatedFrames` struct 定义 + `impl` + `impl Drop`（接收 `state.rs` / `transitions.rs` 剩余的必要方法） |
| `crates/paging/src/lib.rs` | `alloc_node_frame` 新增 `write_bytes(0, PAGE_SIZE)` 清零（页表节点需要全零初态） |

合并后的 `AllocatedFrames` 定义预计约 50 行，集中在 `alloc.rs`（或拆新文件 `frames.rs` 作二级组织）。

### API 变更

| 项 | 变更 |
|----|------|
| `pub use state::{AllocatedFrames, FrameState, Frames}` | 改为 `pub use frames::AllocatedFrames`；`FrameState`/`Frames` 不再导出 |
| `FreeFrames` | 内部过渡消失——`alloc_from_backend` 直接返回 `AllocatedFrames` |
| `AllocatedFrames::alloc` | **不再清零**——返回内容未初始化的帧，调用方负责初始化 |
| `AllocatedFrames` 其他方法 | 不变（`alloc_one`, `count`, `start_paddr`） |

无外部调用方受影响（`FreeFrames` / `FrameState` / `Frames` 本来就无外部使用）。
清零语义变更影响的调用方需要逐一检查并补充初始化逻辑。

### 测试

- `tests/frame-test/src/alloc.rs`：如有对 `Frames<S>` 的类型使用需要调整；否则无变更
- 预计所有现有测试无需修改 fixture——它们操作的是 `AllocatedFrames::alloc` 返回值
- 注意：分配后不清零意味着测试中不能假设内容为零——需检查现有测试是否依赖此假设

### 文档

- `crates/frame_allocator/src/lib.rs` 模块注释：删除 "2-State Typestate" 章节，改为"分配器接口"；明确标注 `alloc` 返回的帧**内容未初始化**
- `docs/design/memory-subsystem-v2.md` §4.1 / §4.2：删除 2-state 状态图和"为什么只有 2 个状态"章节，改为"单一类型 AllocatedFrames + RAII"描述
- README：如有 frame_allocator 的独立 README，同步更新

## 参考

- [ADR-006](006-memory-subsystem-simplification.md) — 4-state → 2-state 简化（本 ADR 进一步简化至 0-state）
- [Rust tracking issue #95174](https://github.com/rust-lang/rust/issues/95174) — `adt_const_params` 稳定性进度
- [Theseus `frame_allocator`](https://github.com/theseus-os/Theseus/tree/theseus_main/kernel/frame_allocator) — 保留 4-state typestate，因为 `Mapped` 对应真实 PTE 状态；SimpleKernel 因全量映射不适用
- [Rust API Guidelines §C-NEWTYPE](https://rust-lang.github.io/api-guidelines/type-safety.html) — newtype 用于表达类型级不变量；本 ADR 判定当前 newtype 层级过度
