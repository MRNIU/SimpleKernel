# ADR-007: 消除 VMA 模块及内存子系统死代码

> **状态**: 提议
>
> **日期**: 2026-04-12
>
> **审计阶段**: 内存子系统精简
>
> **涉及模块**: `crates/memory`, `crates/paging`, `crates/memory_types`, `crates/frame_allocator`

## 背景

ADR-006 将内存子系统从 4-state typestate 简化为 2-state（Free/Allocated），
并确立了 SAS 全量映射 + 权限覆盖模型。但以下遗留问题未在 ADR-006 中处理：

1. **VMA 模块（`memory/vma.rs`）**：从旧的 Theseus 式架构原样搬来，
   ADR-006 仅做了 3 行机械性重命名。VMA 是传统多进程 OS 的 per-process
   虚拟内存区域描述符，在 SAS 单地址空间下严重过度设计——
   其两个实际用途（持有内核段 OwnedPages + MMIO 重叠检测）可用更简单的结构替代。

2. **`mmap_lazy` / `handle_page_fault` / `mprotect`**：与 SAS 全量映射架构矛盾——
   全量映射下无 demand paging，page fault 是内核 bug 而非正常事件；
   `mprotect` 只更新 VMA 元数据而不操作 PTE，是 broken stub。

3. **`unmap_page` / `unmap_page_with_flags`**：SAS 全量映射下 PTE 始终存在，
   权限变更通过 `update_flags` 完成，不需要清除 PTE 和回收页表节点。

4. **`OwnedPages::as_type` / `as_type_mut`**：未被使用的泛型帧访问 API，
   未来 DMA/共享内存场景应设计专用类型（如 `DmaBuffer<T>`）而非通用 byte buffer 视图。

5. **`FrameSpan` / `PageSpan` / `SpanIter`**：`PageSpan` 和 `SpanIter` 无调用者；
   `FrameSpan` 仅在 `frame_allocator` 内部使用，不应暴露在 `memory_types` 公共 API 中。

## 决策

### VMA 替代方案

- 内核段 OwnedPages：通过 `mem::forget` 永久持有——
  权限已写入页表，帧与内核同生命周期，Drop 永不执行
- MMIO 重叠检测：`BTreeMap<VirtAddr, usize>` 静态变量，内联于 `memory/lib.rs`
- `ArchInit::map_early_mmio` 签名删除无用的 `&mut AddressSpace` 参数

### 页表清理

- 删除 `unmap_page` / `unmap_page_with_flags` / `dec_ref`——
  SAS 全量映射不需要取消映射，权限变更由 `update_flags` 负责
- 删除 `as_type` / `as_type_mut` / `pte_flags` / `check_bounds_and_align`（mapping.rs）——
  MMIO 的 `check_bounds_and_align` 内联为 `mmio.rs` 私有函数

### memory_types 清理

- 删除 `PageSpan`（死代码）、`SpanIter` re-export（死代码）
- `FrameSpan` 从 `memory_types` 移到 `frame_allocator` 内部（`pub(crate)` 别名）
- `Frames::range()` 删除（零调用者）

## 理由

**VMA 在 SAS 下不适用**：VMA 的存在前提是多进程 + 多地址空间。SAS 只有一张页表，
帧分配器已防止重复分配，OwnedPages RAII 已防止 use-after-free——
VMA 的地址簿记在 SAS 下是冗余保护。

**`mem::forget` 语义正确**：内核段帧在 `frame_allocator::init` 中被标记为 reserved
（不经过 buddy），与内核同生命周期。`forget` 阻止 Drop（不恢复权限、不归还帧），
这正是永久资源的正确语义。

**`unmap_page` 与全量映射矛盾**：新架构中 PTE 生命周期为
`创建(set_page_flags) → 权限变更(update_flags) → 永远存在`，
没有"清除 PTE"的状态转换。

**YAGNI**：`mmap_lazy`、`handle_page_fault`、`as_type` 等从未被调用且从未被测试。
等实际需求出现时，应根据 SAS 架构重新设计而非复用传统 OS 的接口。

## 影响

- **删除文件**: `memory/vma.rs`、`tests/vma-test/`
- **删除代码**: ~450 行（VMA ~310, unmap ~70, as_type ~50, 别名/re-export ~20）
- **新增代码**: ~30 行（MMIO BTreeMap 跟踪）
- **API 变更**: `memory::init()` 返回 `()` 而非 `AddressSpace`；
  `ArchInit::map_early_mmio()` 不再接受 `&mut AddressSpace`
- **测试**: 删除 vma-test（8 个）、paging-test 中 6 个 unmap 测试；
  剩余 20 个系统测试全部通过

## 参考

- ADR-005: SAS 全量映射 + OwnedPages 模型
- ADR-006: 2-state typestate 简化
- [Theseus OSDI'20](https://www.usenix.org/system/files/osdi20-boos.pdf) — VMA 原始参考
- [Singularity MSR'05](https://www.microsoft.com/en-us/research/project/singularity/) — SAS 架构无需 VMA 的先例
