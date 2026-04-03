# 审计进度

> 此文件由 AI 在每次审计对话结束时自动更新，用于跨对话传递上下文。
> 请勿手动编辑，除非需要纠正 AI 的记录。

## 当前状态

**当前 Phase**: R3 — 内存子系统（进行中）
**下一个目标**: R3 剩余工作（README 补全）或 R2 — 同步与 Per-CPU

## 上次对话摘要

**日期**：2026-04-03

### 已完成

- R3 审查报告：审阅 `frame_allocator`、`page_allocator`、`page_table_entry`、`paging`、`tlb`、`heap`、`memory` 七个 crate（~4656 行）
- R3 实施重构：
  - `frame_allocator`: 二态 -> 四态 typestate（新增 `Mapped`/`Unmapped` 状态，`MappedFrames::Drop` panic 防泄漏）
  - `frame_allocator`: 移除 `Frames::split_at`/`merge`（未使用）
  - `page_allocator`: **整个 crate 删除**（ADR-003: SAS 下只需 identity mapping，VA == PA，虚拟页分配器多余）
  - `paging/mapping.rs`: `MappedPages` 持有 `ManuallyDrop<MappedFrames>`，`map(frames, flags)` 签名简化（VA 从 PA 推导）
  - `paging/mmio.rs`: `MmioRegion` 移除 `AllocatedPages` 字段
  - `paging/lib.rs`: `KERNEL_PT_PTR` AtomicUsize -> `KERNEL_PAGE_TABLE` spin::Once（值直接在 .bss，消除 Box::leak + unsafe）；`set_kernel_page_table` -> `init_kernel_page_table`（safe）
  - `paging/error.rs`: `AlreadyMapped` 拆分为 `AlreadyMappedIdentical`（幂等）/ `AlreadyMappedConflict`（冲突）
  - `paging/table.rs`: `identity_map_range` 容忍幂等重复映射（同一页内多个 MMIO 设备不再 panic）
  - `memory/vma.rs`: 移除 `VmaKind`，合并 `mmap_identity`/`mmap_anonymous` 为 `mmap`；`RegionOverlap` 拆分为 `RegionIdentical`/`RegionOverlap`
  - `memory/init.rs`: 移除 `page_allocator::init`，简化段映射
  - `memory/lib.rs`: 移除 `page_allocator` re-export，`map_mmio` 中 `forget` -> `ManuallyDrop`，MMIO VMA 注册容忍 `RegionIdentical`
  - `heap/lib.rs`: 锁级别 `UNSPECIFIED` -> `HEAP`
  - `page_table_entry/riscv64.rs`: W+R 检查 `debug_assert` -> `assert`
  - `sync/lock_stack.rs`: 新增 `KERNEL_AS`/`KERNEL_PT`/`DMA`/`PANIC` 锁级别常量
  - `memory/globals.rs`: 锁级别 `UNSPECIFIED` -> `KERNEL_AS`
  - `src/panic.rs`: 锁级别 `UNSPECIFIED` -> `PANIC`
  - `src/device/hal.rs`: 锁级别 `UNSPECIFIED` -> `DMA`

### 关键决策

| # | 决策 | 状态 | ADR |
|---|------|------|-----|
| 四态 typestate | Free/Allocated/Mapped/Unmapped，Mapped Drop panic | 已实施 | — |
| 移除 page_allocator | SAS 下只需 identity mapping，VA == PA | 已实施 | ADR-003 |
| 移除 split/merge | `Frames`/`Pages`/`MappedPages` 的 split/merge 未使用 | 已实施 | — |
| KERNEL_PT_PTR -> spin::Once | 值直接在 .bss，消除 Box::leak | 已实施 | — |
| AlreadyMapped 拆分 | 幂等（Identical）vs 冲突（Conflict）| 已实施 | — |
| RegionOverlap 拆分 | 完全重合（Identical）vs 部分重叠（Overlap）| 已实施 | — |
| 锁级别补全 | KERNEL_AS=3, KERNEL_PT=4, DMA=5, PANIC=100 | 已实施 | — |
| heap feature gate | `sync_unsafe_cell` const fn 仍未稳定，保留 | 已决定 | — |

### 未决设计问题

- R6 范围的锁（`dev_mgr`、`ramfs`、`fd`、`virtio_blk`、`mount_table`）锁级别待 R6 审计分配
- `mmap_identity`/`mmap_anonymous` 合并后的 `mmap` 接口中 `mmap_lazy` 的 `handle_page_fault` 路径未经裸机测试验证

### R8 待办（审计收尾阶段）

- [ ] `CONTRIBUTING.md` — 贡献指南
- [ ] `CODE_OF_CONDUCT.md` — 社区行为准则
- [ ] `SECURITY.md` — 安全漏洞报告流程
- [ ] `paging/README.md` — 分页子系统文档
- [ ] `tlb/README.md` — TLB 管理文档
- [ ] `heap/README.md` — 堆分配器文档
- [ ] `memory/README.md` — 内存门面 crate 文档

## 已完成的目标

| 日期 | Phase | 内容 |
|------|-------|------|
| 2026-04-03 | R0 | 审查报告 + 基础设施实施（CI/文档/审计基线/依赖/ADR） |
| 2026-04-03 | R1 | 审查报告 + 实施修复（span/config/build_common/memory_types） |
| 2026-04-03 | R3 | 审查报告 + 重构实施（四态 typestate、移除 page_allocator、锁级别、幂等映射） |
