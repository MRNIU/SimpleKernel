# memory

内核内存管理门面——统一编排子系统初始化、提供 MMIO 映射接口。

## 概览

`memory` 是内存子系统的**策略层**，对外暴露三个公开入口：`init()` / `init_smp()` / `map_mmio()`。
具体机制（页表、帧分配、TLB、PTE 编解码）由下层 crate 实现，本 crate 只负责编排顺序和做跨模块校验。

## 子系统分层

```
┌─────────────────────────────────────────────────────┐
│  策略层 — memory (本 crate)                          │
│  init() · init_smp() · map_mmio()                    │
├─────────────────────────────────────────────────────┤
│  机制层 — paging                                     │
│  PageTable · MmioRegion                              │
├───────────────┬──────────────────┬──────────────────┤
│ frame_allocator│ page_table_entry  │ tlb             │
│ 物理帧分配     │ PTE 编解码        │ TLB 刷新        │
│ AllocatedFrames│ PteFlagsOps/PteOps│ TlbFlushGuard   │
├───────────────┴──────────────────┴──────────────────┤
│  memory_types — PhysAddr · VirtAddr · Frame · Span   │
└─────────────────────────────────────────────────────┘
```

## SAS 全量映射模型

SAS 架构下所有物理内存在 boot 时 identity-map 为 `kernel_rw`（背景层），
运行时只调整权限（覆盖层），永远不创建或删除 PTE。

## 错误策略

SAS 下分页/MMIO 映射失败都是**内核 bug**（boot 时 OOM、RAM/Device 重叠、
flags 冲突），因此 `map_mmio` 和下层页表操作**失败即 panic**。
唯一向上传递 `FrameAllocError::OutOfMemory` 的场景是运行时帧分配
（`AllocatedFrames::alloc`），不经过本 crate 的接口。

## 典型使用方式

```rust,ignore
use frame_allocator::AllocatedFrames;
use paging::{PteFlags, PteFlagsOps, kernel_page_table};

// 分配帧 + 设置只读权限（适用于内核段初始化这种永久持有场景）
let frames = AllocatedFrames::alloc(4)?;
let va = frames.start_paddr().to_virt();
kernel_page_table().update_range_flags(va, frames.page_count(), PteFlags::kernel_ro());
core::mem::forget(frames); // 永久持有——阻止 buddy 回收

// MMIO 映射（失败 panic）
let region = memory::map_mmio(PhysAddr::new(0x1000_0000), 0x1000);
```
