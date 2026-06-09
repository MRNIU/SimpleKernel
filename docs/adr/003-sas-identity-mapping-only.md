<!-- Copyright The SimpleKernel Contributors -->

# ADR-003: SAS 架构下只支持 identity mapping

> **状态**: 已接受
>
> **日期**: 2026-04-03
>
> **审计阶段**: R3 — 内存子系统
>
> **涉及模块**: `crates/page_allocator`, `crates/paging`, `crates/memory`, `crates/frame_allocator`

## 背景

SimpleKernel 采用单地址空间（SAS）架构——所有代码运行在同一特权级和地址空间中，
不存在用户态/内核态分离，隔离通过 Rust 类型系统 + crate 可见性实现。

当前内存子系统同时支持 identity mapping（VA == PA）和非 identity mapping（VA != PA）：

- `page_allocator` 独立管理虚拟地址空间，与 `frame_allocator`（物理帧）分离
- `MappedPages::map(pages, frames, flags)` 接受独立的 `AllocatedPages`（VA）和 `AllocatedFrames`（PA）
- `AddressSpace` 提供 `mmap_anonymous`（VA != PA）和 `mmap_identity`（VA == PA）两种接口

然而，审计发现**内核代码中全部是 identity mapping，没有任何 VA != PA 的实际使用**。

非 identity mapping 的本质是为**多个互不信任的执行体**共享物理内存而设计的。
SAS 架构的核心主张是用编译器替代 MMU 做隔离（[Theseus OSDI'20]: "the compiler
is the protection ring"）。SAS 内核的演进路径是强化语言级安全（`#![forbid(unsafe_code)]`、
capability token、编译期可见性），而非回到地址翻译。如果需要运行不受信任的二进制，
那就不再是 SAS——需要从根本上重新设计，不是加一个 `page_allocator` 能解决的。

## 备选方案

### 方案 A: 移除 page_allocator，只保留 identity mapping

移除 `page_allocator` crate。`MappedPages` 只接受 `AllocatedFrames`，
VA 与 PA 数值相同（identity mapping）。`AddressSpace` 合并为单一映射接口。

**优点**:
- 消除 ~400 行不使用的代码（`page_allocator` 及其 typestate）
- `MappedPages::map` 签名简化为 `map(frames, flags)`
- `AllocatedPages` 类型消失，减少一层所有权追踪
- 与 SAS 架构一致——不维护不使用的抽象
- VA 空间占用由 `frame_allocator` + `AddressSpace` VMA 列表 + 页表三重保障，无冗余

**缺点**:
- 重构工作量中等——涉及 `paging`、`memory`、所有调用方

### 方案 B: 保持现状

不做变更。

**优点**:
- 零工作量

**缺点**:
- 维护不使用的代码路径（`mmap_anonymous` 的 VA != PA 场景无裸机测试）
- 违反项目原则"不为假设性需求设计"

## 决策

选择 **方案 A**。

SAS 架构下非 identity mapping 没有实际用途，移除 `page_allocator` 简化内存子系统。

## 影响

- **代码变更**:
  - 删除 `crates/page_allocator/` 整个 crate
  - `paging/mapping.rs`: `MappedPages` 移除 `AllocatedPages` 字段，VA 从 PA 推导
  - `paging/mmio.rs`: `MmioRegion` 移除 `AllocatedPages` 字段
  - `memory/vma.rs`: 合并 `mmap_anonymous` / `mmap_identity` 为 `mmap`
  - `memory/init.rs`: 移除 `page_allocator::init` 调用
  - 根 `Cargo.toml`: 移除 workspace member
- **API 变更**: `MappedPages::map` 签名变更，`unmap` 返回值变更
- **测试**: `paging` 和 `memory` 测试适配新接口
- **文档**: `AGENTS.md` CODE MAP、frame_allocator AGENTS

## 参考

- [Theseus OSDI'20](https://www.usenix.org/system/files/osdi20-boos.pdf) — SAS + identity mapping
- [Singularity MSR](https://www.microsoft.com/en-us/research/project/singularity/) — SAS + identity mapping
- [SPIN SOSP'95](https://cseweb.ucsd.edu/~savage/papers/Sosp95.pdf) — 语言安全 SAS 内核
