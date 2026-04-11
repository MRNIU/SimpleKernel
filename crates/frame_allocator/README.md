# frame_allocator

物理帧分配器——以 2-state typestate 在编译期追踪帧的所有权。

## 概览

`frame_allocator` 管理内核的物理页帧（4KB 粒度），提供分配和释放操作。
核心设计是将帧的所有权状态（空闲 / 已分配）编码为 Rust 的 const generic 类型参数，
使非法的状态转换在编译期被拒绝。

从 `memory` crate 独立出来的原因：帧分配是内存子系统中最底层、最独立的能力，
被页表（`paging`）、权限管理（`OwnedPages`）、VMA 等上层模块共同依赖。
独立 crate 使依赖方向单向化，也允许 `paging` 直接集成，无需经过 `memory`。

## 状态机

核心类型定义：

```rust
pub enum FrameState { Free, Allocated }

pub struct Frames<const S: FrameState> {
    range: FrameSpan,   // 连续物理帧范围 [start, end)
}

pub type FreeFrames      = Frames<{ FrameState::Free }>;
pub type AllocatedFrames = Frames<{ FrameState::Allocated }>;
```

只有两个状态——帧在分配器中（Free）或被外部持有（Allocated）：

```
            alloc()              Drop
buddy pool ────────> Allocated ────────> buddy pool
            (Free->Allocated)     (dealloc_to_backend)
```

`FrameState` 通过 const generic 参数嵌入类型，不同状态是**不同类型**。
状态转换消费 self 并返回新状态的实例，编译器自动阻止对旧实例的使用。

| 状态 | 含义 | 所有者 | Drop 行为 |
|------|------|--------|-----------|
| `Free` | 刚从 buddy allocator 取出，尚未交给用户 | 分配器内部 | 归还 buddy |
| `Allocated` | 用户持有 | 调用方 / OwnedPages | 归还 buddy |

Drop 行为统一——所有状态的帧都归还分配器，无需运行时 panic 保护网。

## 模块结构

```
src/
├── lib.rs           crate 入口，pub use 汇总
├── error.rs         FrameAllocError 定义
├── state.rs         FrameState 枚举、Frames<S> 结构体、通用操作、Drop
├── alloc.rs         全局 SpinLockIrq<FrameAllocator<32>>、init / alloc / dealloc
└── transitions.rs   FreeFrames::into_allocated() + AllocatedFrames::alloc()
```

## 分配与释放路径

### 分配

```
AllocatedFrames::alloc(count)
  |
  +-> alloc_from_backend(count)          <- 持有 SpinLockIrq
  |     +-> buddy.alloc(count)
  |     +-> 构造 FreeFrames              <- 释放锁
  |
  +-> write_bytes(ptr, 0, ...)           <- 零初始化（锁外执行）
  |
  +-> FreeFrames::into_state()           <- typestate 转换（零开销）
```

关键设计：零初始化在锁外执行，避免持锁期间做 O(n) 的内存写入。

### 释放

```
drop(AllocatedFrames)  或  drop(FreeFrames)
  |
  +-> dealloc_to_backend(range)          <- 持有 SpinLockIrq
        +-> buddy.dealloc(start, count)
```

## 锁与中断安全

全局分配器使用 `SpinLockIrq`（获取时关中断，释放时恢复），而非普通 `SpinLock`。

**原因：** 帧分配可能在中断上下文中被调用（如 page fault handler 分配新帧）。
如果使用普通 `SpinLock`：

1. 线程持有锁 -> 中断到来 -> 同核心进入 handler
2. Handler 尝试分配帧 -> 拿同一把锁 -> **死锁**

`SpinLockIrq` 在获取锁前禁用中断，消除了这一场景。

## 使用示例

```rust
// 分配 1 帧（4KB），内容已清零
let frame = AllocatedFrames::alloc_one()?;
let pa = frame.start_paddr();

// 分配 4 个连续帧
let frames = AllocatedFrames::alloc(4)?;
assert_eq!(frames.count(), 4);

// drop 时自动归还 buddy allocator
```

配合 `OwnedPages` 设置 PTE 权限：

```rust
let frames = AllocatedFrames::alloc(4)?;
let guard = OwnedPages::new(frames, PteFlags::kernel_ro());
// guard.set_flags(PteFlags::kernel_rw());  // 改权限
// drop(guard) → 恢复默认权限 + 释放帧
```

## 注意事项

### 1. 所有分配均零初始化

`alloc()` / `alloc_one()` 始终将帧内容清零，防止信息泄漏。

### 2. Buddy 内部碎片

`alloc(count)` 内部将 `count` 向上取整到 2 的幂次。例如 `alloc(3)` 实际
分配 4 帧。多出的帧在分配期间不可用，释放后自动合并回 buddy。

### 3. 初始化顺序依赖

`frame_allocator::init()` 必须在堆初始化之后、任何帧分配之前调用。
buddy allocator 内部使用 `BTreeSet`（堆分配），因此依赖堆可用。
