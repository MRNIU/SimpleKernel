# 架构决策记录（ADR）

本目录是 SimpleKernel 的 ADR（Architecture Decision Record，架构决策记录）目录，记录审计、重构和功能演进过程中的重要架构决策。

ADR 描述“为什么做出某个决策”。当前架构和当前设计仍应沉淀到 `docs/design/` 中的 SAD/SDD 或子系统设计文档；ADR 不替代当前状态文档。

## 模板

新建 ADR 时，复制 `docs/templates/adr-template.md`，文件命名为 `NNN-简短描述.md`（如 `014-mmio-region-lifetime.md`）。

## 索引

| 编号 | 标题 | 状态 | 日期 | 阶段 |
|------|------|------|------|------|
| 001 | [AArch64 浮点支持](001-aarch64-float-support.md) | 提议 | — | — |
| 002 | [RISC-V tp 寄存器 per-CPU vs TLS](002-tp-register-percpu-vs-tls.md) | 提议 | — | R2 |
| 003 | [SAS 架构下是否只支持 identity mapping](003-sas-identity-mapping-only.md) | 提议 | 2026-04-03 | R3 |
| 004 | [消除内核源码中的 `#[cfg(bare_metal)]`](004-cfg-bare-metal-elimination.md) | 提议 | 2026-04-04 | — |
| 005 | [SAS 全量映射 + OwnedPages 所有权模型](005-sas-full-mapping-owned-pages.md) | 已取代 | 2026-04-09 | R3 |
| 006 | [内存子系统简化——4KB 单页 + 2-state + 权限覆盖模型](006-memory-subsystem-simplification.md) | 已接受 | 2026-04-11 | R3 |
| 007 | [消除 VMA 模块及内存子系统死代码](007-eliminate-vma-and-dead-code.md) | 提议 | 2026-04-12 | R3 |
| 008 | [删除 FrameState typestate 与 `adt_const_params` nightly 依赖](008-eliminate-frame-state-typestate.md) | 提议 | 2026-04-17 | R3 回看 |
| 009 | [删除 CLAIMED 软件位，保留 page poison](009-remove-claimed-bit-and-page-poison.md) | 提议 | 2026-04-17 | R3 回看 |
| 010 | [PageTable 内部结构简化——BTreeMap → Vec，删除引用计数死代码](010-pagetable-nodes-vec-and-remove-refcount.md) | 提议 | 2026-04-17 | R3 回看 |
| 011 | [MMIO overlap 检测统一至 `create_pte`；引入 `FlagsConflict` 错误](011-mmio-overlap-via-set-page-flags.md) | 提议 | 2026-04-17 | R3 回看 |
| 012 | [PageTable 拆分；hot-path PTE 更新无锁化](012-pagetable-split-lock-free-hot-path.md) | 提议 | 2026-04-17 | R3 回看 |
| 013 | [删除 `OwnedPages` 抽象层](013-ownedpages-necessity.md) | 已接受 | 2026-04-18 | R3 回看 |

## 状态规则

- AI 生成的 ADR 状态**必须为"提议"**。只有项目作者 review 后才可改为"已接受"。
- "已废弃"和"已取代"同样只能由项目作者标记。
- ADR 被接受后，如影响当前架构或设计，必须同步更新 `docs/design/` 中对应 SAD/SDD 或子系统设计文档。

## 何时需要写 ADR

- 在两个以上合理方案中做了选择（如 `ManuallyDrop` vs `mem::forget`）
- 改变了现有设计的方向（如从 `dyn Trait` 改为 enum dispatch）
- 引入了新的 Rust 范式（如 typestate 编码状态机）
- 决定**不做**某件事（如决定不引入 RwLock），且理由不显而易见
- 从既有代码中发现未文档化但必须长期保持的架构不变量
