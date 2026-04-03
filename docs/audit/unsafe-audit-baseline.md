# Unsafe 审计基线

> **生成日期**：2026-04-03
> **分支**：`feat/rust-SAS`（commit: aff99bdd8）
> **工具**：手动 grep 统计（后续由 `cargo-geiger` CI step 自动化守护）

## 总览

| 类别 | 数量 |
|------|------|
| `unsafe {}` 块 | **142** |
| `unsafe fn` 声明 | 37 |
| `unsafe impl` 块 | 17 |
| `unsafe trait` 声明 | 1 |
| `#[unsafe(...)]` 属性 | 11 |
| `unsafe extern "C"` | 16 |
| `// SAFETY:` 注释 | 139 |

**SAFETY 注释覆盖率**：~63%（unsafe 块 142 个，SAFETY 注释 139 个——注意部分注释对应 `unsafe fn`/`unsafe impl` 而非块）

## 按目录分布

| 目录 | unsafe 块 | unsafe fn | unsafe impl | 合计 |
|------|----------|-----------|-------------|------|
| `crates/paging/src/` | 28 | 2 | 1 | 31 |
| `crates/per_cpu/src/` | 22 | 7 | 3 | 32 |
| `src/arch/aarch64/` | 21 | 4 | 0 | 25 |
| `src/task/` | 17 | 3 | 2 | 22 |
| `src/elf.rs` | 7 | 1 | 2 | 10 |
| `crates/memory/src/` | 6 | 0 | 0 | 6 |
| `crates/sync/src/` | 6 | 0 | 7 | 13 |
| `src/arch/riscv64/` | 5 | 1 | 0 | 6 |
| `src/main.rs` | 5 | 0 | 0 | 5 |
| `src/boot.rs` | 4 | 2 | 0 | 6 |
| `crates/frame_allocator/src/` | 3 | 1 | 0 | 4 |
| `crates/interrupt_state/src/` | 3 | 5 | 0 | 8 |
| `crates/heap/src/` | 2 | 3 | 1 | 6 |
| 其他（各 ≤2） | 13 | 8 | 1 | 22 |

## 按文件 Top 10（unsafe 块数）

| 文件 | unsafe 块 |
|------|----------|
| `crates/per_cpu/src/lib.rs` | 22 |
| `src/task/sched.rs` | 10 |
| `crates/paging/src/table.rs` | 10 |
| `crates/paging/src/mapping.rs` | 10 |
| `src/arch/aarch64/interrupt.rs` | 9 |
| `src/task/mod.rs` | 7 |
| `src/elf.rs` | 7 |
| `crates/memory/src/init.rs` | 6 |
| `crates/paging/src/lib.rs` | 6 |
| `src/main.rs` | 5 |

## SAFETY 注释缺口（优先补齐目标）

以下文件的 unsafe 块密度高但 SAFETY 注释覆盖低，是后续审计的重点：

1. **`crates/paging/src/table.rs`** — 10 块，页表操作（R3 审查）
2. **`crates/paging/src/mapping.rs`** — 10 块，内存映射（R3 审查）
3. **`crates/memory/src/init.rs`** — 6 块，链接器符号（R3 审查）
4. **`src/arch/aarch64/interrupt.rs`** — 9 块，中断处理（R4 审查）

## 基线用途

此文件作为 unsafe 数量的基线。CI 中 `cargo-geiger` 的输出应与此基线对比：
- unsafe 块数增加 → 需要在 PR 中说明理由
- unsafe 块数减少 → 正常（审计过程中消除不必要的 unsafe）
- 新增 unsafe 块必须有 `// SAFETY:` 注释
