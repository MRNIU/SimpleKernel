# frame_allocator

物理帧分配器——RAII 所有权追踪 + buddy 后端。

## 概览

`frame_allocator` 管理内核的物理页帧（4KB 粒度），提供分配和释放操作。
核心类型 `AllocatedFrames` 通过 Rust 的 move 语义在编译期追踪帧所有权——
不可 Clone、不可 Copy，Drop 时自动归还 buddy allocator。

从 `memory` crate 独立出来的原因：帧分配是内存子系统中最底层、最独立的能力，
被页表（`paging`）、堆扩展、DMA、内核段覆盖等上层模块共同依赖。
独立 crate 使依赖方向单向化，也允许 `paging` 直接集成，无需经过 `memory`。

## 核心类型

```rust
pub struct AllocatedFrames {
    range: FrameSpan,   // 连续物理帧范围 [start, end)
}
```

```
            alloc()              Drop
buddy pool ────────> AllocatedFrames ────────> buddy pool
                                      (dealloc_to_backend)
```

Rust ownership + Drop 提供所有编译期保证：

| 防御场景 | 机制 |
|---------|------|
| 未分配就使用帧 | `AllocatedFrames::alloc` 是唯一公开构造器 |
| 释放后使用帧 | Drop 消费 self → 编译错误 |
| 双重持有 | 不可 Clone/Copy → move 语义保证唯一所有权 |

## 模块结构

```
src/
├── lib.rs           crate 入口，pub use 汇总
├── error.rs         FrameAllocError 定义
├── frames.rs        AllocatedFrames 结构体、alloc/alloc_one、Drop
└── alloc.rs         全局 SpinLockIrq<FrameAllocator<32>>、init / alloc_from_backend / dealloc
```

## 分配与释放路径

### 分配

```
AllocatedFrames::alloc(count)
  |
  +-> alloc_from_backend(count)          <- 持有 SpinLockIrq
        +-> buddy.alloc(count)
        +-> 构造 AllocatedFrames         <- 释放锁
```

帧内容**未初始化**——调用方按场景决定初始化策略：

| 调用方 | 初始化策略 | 原因 |
|-------|-----------|------|
| `alloc_node_frame()`（页表节点） | 清零 | 无效 PTE = 0 |
| `dma_alloc()`（DMA 缓冲区） | 清零 | 防信息泄漏到设备 |
| 栈/一般数据 | 按需 | 立即写入，无需清零 |

这遵循"机制与策略分离"原则（与 Linux `alloc_pages` / Theseus `allocate_frames` 一致）。

### 释放

```
drop(AllocatedFrames)
  |
  +-> dealloc_to_backend(range)          <- 持有 SpinLockIrq
        +-> buddy.dealloc(start, count)
```

## 锁与中断安全

全局分配器使用 `SpinLockIrq`（获取时关中断，释放时恢复），而非普通 `SpinLock`。

**原因：** 普通线程路径可能正在持有 frame allocator 锁，此时若同核心中断进入并
再次拿同一把锁，会造成递归死锁。`SpinLockIrq` 在获取锁前禁用中断，消除了这一场景：

1. 线程持有锁 -> 中断到来 -> 同核心进入 handler
2. Handler 尝试分配帧 -> 拿同一把锁 -> **死锁**

但这不表示 frame allocator 承诺可在 hard IRQ 中分配。当前 buddy 后端的元数据
依赖堆结构，而 `heap` crate 已明确禁止中断上下文堆操作。因此：

- `alloc_from_backend()` 会断言当前不在中断上下文中。
- `dealloc_to_backend()` 同样会断言当前不在中断上下文中。
- hard IRQ handler 不能直接分配/释放 `AllocatedFrames`；如未来确实需要，应先设计
  no-heap frame metadata 或 per-CPU emergency frame cache。

## 使用示例

```rust
// 分配 1 帧（4KB），内容未初始化
let frame = AllocatedFrames::alloc_one()?;
let pa = frame.start_paddr();

// 分配 4 个连续帧
let frames = AllocatedFrames::alloc(4)?;
assert_eq!(frames.page_count(), 4);

// drop 时自动归还 buddy allocator
```

配合 `PageTable::update_range_flags` 设置 PTE 权限：

```rust
let frames = AllocatedFrames::alloc(4)?;
let va = frames.start_paddr().to_virt();
paging::kernel_page_table().update_range_flags(va, frames.page_count(), PteFlags::kernel_ro());
// drop(frames) → 自动归还 buddy
// 若需永久持有（如内核段），显式 core::mem::forget(frames)
```

## 注意事项

### 1. 帧内容未初始化

`alloc()` / `alloc_one()` 返回的帧内容未初始化——调用方必须按需初始化。
在 Rust 中访问未初始化内存需要 `unsafe`，编写者有义务保证初始化。

### 2. Buddy 内部碎片

`alloc(count)` 内部将 `count` 向上取整到 2 的幂次。例如 `alloc(3)` 实际
分配 4 帧。多出的帧在分配期间不可用，释放后自动合并回 buddy。

### 3. 初始化顺序依赖

`frame_allocator::init()` 必须在堆初始化之后、任何帧分配之前调用。
buddy allocator 内部使用 `BTreeSet`（堆分配），因此依赖堆可用。

`reserved` 参数只做页对齐、溢出和重叠校验，并记录启动日志；它不会从
`free_start` / `free_size` 描述的空闲范围中扣除页面。调用方必须先把所有
固件区、内核镜像区和其他保留区从 free 范围中排除，再调用 `init()`。
