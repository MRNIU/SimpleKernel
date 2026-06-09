<!-- Copyright The SimpleKernel Contributors -->

# ADR-005: SAS 全量映射 + OwnedPages 所有权模型

> **状态**: 已取代（被 [ADR-006](006-memory-subsystem-simplification.md) 取代并细化）
>
> **日期**: 2026-04-09
>
> **审计阶段**: R3 — 内存子系统
>
> **涉及模块**: `crates/frame_allocator`, `crates/paging`, `crates/memory`

## 背景

SimpleKernel 采用 SAS（单地址空间）架构，隔离通过 Rust 类型系统 + crate 可见性实现。
当前内存子系统存在以下问题：

1. **内部矛盾**：boot 全量映射物理内存（含空闲帧区域），但 `MappedPages::map()` 对已有 PTE panic（`AlreadyMappedIdentical`），typestate 声称 Free 帧无 PTE 但实际有。
2. **safety 契约违反**：`frame_allocator::init` 文档要求 "free 和 reserved 不重叠"，但 `data_pages = (mem_end - rodata_end)` 包含了 free pool，两者重叠。
3. **buddy_system_allocator 的 `Heap` 类型在空闲帧内存中存链表指针**：依赖全量映射才能工作，与 typestate "Free 帧无 PTE" 矛盾。（注：物理帧分配器使用的 `FrameAllocator` 类型将元数据存储在堆上的 BTreeSet 中，不触碰帧内存。此条仅适用于 `Heap` 类型。）
4. **`MappedPages::drop` 删除 PTE**：在全量映射模型下会在背景映射中留下空洞。

对其它内核的调研结论：
- Linux / FreeBSD / seL4 / Redox / rCore 均使用全量/直接映射，页表节点通过直接映射访问
- Theseus 不做全量映射，但需要递归页表（RISC-V Sv39 不支持）或 fixmap（实现复杂）
- SimpleKernel 的 C++ 前身版本：全量映射 + 堆分配页表节点

## 备选方案

### 方案 A: Theseus 路线——不全量映射 + typestate 字面追踪 PTE

只映射内核段，空闲帧无 PTE。`MappedPages::map` 真正建立 PTE，`drop` 真正删除。

**优点**:
- typestate 语义字面正确
- 空闲帧无 PTE，未来可细粒度控制权限

**缺点**:
- 页表节点帧分配后无 PTE → 鸡生蛋问题
- 需要 fixmap（两架构各实现一遍，TLB flush 开销）或递归页表（RISC-V 不支持）
- 或节点帧从堆分配（两套分配体系，语义不统一）

### 方案 B: 全量映射 + OwnedPages 所有权模型

boot 映射全部物理内存（背景层）。`MappedPages` 改名 `OwnedPages`，管理所有权和权限覆盖，不管 PTE 有无。

**优点**:
- 和 Linux / rCore / C++ 版本一致，成熟模式
- 无鸡生蛋问题——所有帧始终可通过 identity mapping 访问
- 页表节点从帧分配器统一分配，无两套体系
- typestate 追踪所有权（谁持有帧），语义自洽
- 页表仍然提供硬件级纵深防御（NX / RO / guard page / MMIO 属性）

**缺点**:
- 空闲帧有 PTE（默认 kernel_rw），但这在 SAS 下不是安全问题——APP 受 `#![forbid(unsafe_code)]` + crate 可见性约束，无法构造裸指针访问
- 分配器性能：O(log n)（buddy system，已重新采用 `buddy_system_allocator::FrameAllocator<32>`）
- `OwnedPages` 名称与 Theseus 的 `MappedPages` 不同，增加理解成本

### 方案 C: 保持现状 + 打补丁

仅修复 `AlreadyMappedIdentical` panic，不改架构。

**优点**:
- 改动最小

**缺点**:
- 内部矛盾（typestate vs 全量映射）不解决
- `drop` 仍删 PTE，导致背景映射空洞
- safety 契约违反不修复

## 决策

选择 **方案 B**——全量映射 + OwnedPages 所有权模型。

SAS 架构下，隔离由 Rust 类型系统承担，页表的角色是纵深防御（NX/RO/guard page）和 MMIO 映射。全量映射是自然选择——与 Linux 的 direct map、rCore 的 identity map、C++ 版本的设计一致。typestate 追踪帧所有权（而非 PTE 生命周期），语义自洽。

## 理由

- **方案 A 不可行**：RISC-V Sv39 不支持递归页表（非叶 PTE R=W=X=0 在末级被视为无效），fixmap 两架构实现成本高且 TLB 开销显著。
- **方案 C 治标不治本**：typestate 声称 Free 帧无 PTE 但实际全量映射，drop 删 PTE 导致空洞——根本矛盾不消除，后续开发持续踩坑。
- **方案 B 依赖的假设**：
  - SAS 架构不变（若引入硬件级进程隔离，需重新评估）
  - APP 遵守 `#![forbid(unsafe_code)]`（若需运行不可信本地代码，需额外机制）
  - 物理内存 ≤ 虚拟地址空间（Sv39: 512GB，AArch64-48bit: 256TB）

## 影响

- **代码变更**:
  - `crates/frame_allocator/`: 重新采用 `buddy_system_allocator::FrameAllocator<32>`（外部 BTreeSet 元数据，不触碰帧内存），修复 reserved/free 边界
  - `crates/paging/src/mapping.rs`: `MappedPages` → `OwnedPages`，map 接受已有 PTE，drop 恢复默认权限
  - `crates/paging/src/table.rs`: `map_page` / `map_at_level` 处理同 PA 映射
  - `crates/paging/src/lib.rs`: 移除 `KernelNodeFrame::alloc` 冗余手动清零（`AllocatedFrames::alloc_one` 已清零）
  - `crates/memory/src/init.rs`: 分离 reserved/free 边界，添加背景映射
  - `crates/memory/src/lib.rs`, `vma.rs`: re-export 改名
  - `tests/paging-test/`: 更新测试预期
- **API 变更**: `MappedPages` → `OwnedPages`（pub 类型改名）
- **测试**: `test_drop_unmaps` 改为测试 flags 恢复；新增 `test_map_preexisting` 测试幂等/flags 更新
- **文档**: typestate 文档、AGENTS.md 更新

## 参考

- [Linux: `arch/arm64/mm/mmu.c`] — fixmap 早期启动 + direct map 正常运行
- [rCore-Tutorial-v3: `os/src/mm/page_table.rs`] — identity map 全量映射 + `PhysPageNum::get_pte_array()` 直接转指针
- [Theseus OSDI'20 §4.2] — "the Rust compiler is the protection ring"
- [Singularity MSR'05] — SAS + 语言安全隔离，量化 SAS 性能优势
- [SimpleKernel C++ 版: `src/memory/virtual_memory.cpp`] — `aligned_alloc` 页表节点 + 全量 `MapMMIO`
