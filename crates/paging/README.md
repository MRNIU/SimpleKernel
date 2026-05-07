# paging

分页子系统——多级页表管理。

## 概览

本 crate 是内存子系统的**机制层**——提供页表操作原语，但不决定"何时"或
"为什么"映射。策略层（`memory` crate）编排初始化、MMIO 类型化包装与
全局内存布局。应用层通过 `PageTable` 的直接方法使用。

```
memory (策略: 何时映射, MMIO 类型, MEMORY_INFO)
   │
   ▼
paging (机制: 如何映射)  ← 本 crate
   │
   ├── frame_allocator (帧分配)
   ├── page_table_entry (PTE 编解码)
   └── tlb (TLB 刷新)
```

## 核心类型

- `PageTable`：多级基数树页表，管理 PTE 的创建与更新
  - `identity_map_range(start, end, flags)`：建立背景 identity mapping
  - `update_range_flags(va, count, flags)`：修改已映射页权限并刷新 TLB
  - `get_mapping(va)`：查询 PTE

## 帧所有权

帧由 `frame_allocator::AllocatedFrames`（RAII，Drop 时归还 buddy）直接管理。
权限设定是对已映射 PTE 的幂等操作，不与帧生命周期耦合——内核段通过
`update_range_flags` 设置权限后 `mem::forget(frames)` 永久持有（见
[ADR-013](../../docs/adr/013-ownedpages-necessity.md)）。

## 错误策略

SAS 架构下分页操作的失败路径都是**预期外的内核 bug**——
OOM 发生在 boot 时不应出现、映射冲突是调用方逻辑错误、
`PageNotMapped` 违反 SAS 背景层不变量。

因此本 crate 的所有接口**失败即 panic**，不返回 `Result`。
唯一向上传递 `FrameAllocError::OutOfMemory` 的场景在运行时帧分配
（`AllocatedFrames::alloc`），不在本 crate 的接口范围。
