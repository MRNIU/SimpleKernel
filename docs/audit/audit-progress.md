# 审计进度

> 此文件由 AI 在每次审计对话结束时自动更新，用于跨对话传递上下文。
> 请勿手动编辑，除非需要纠正 AI 的记录。

## 当前状态

**当前 Phase**: R1 — 原语层（已完成）
**下一个目标**: R2 — 同步与 Per-CPU

## 上次对话摘要

**日期**：2026-04-03

### 已完成

- R1 审查报告：审阅 `span`、`config`、`build_common`、`memory_types` 四个叶子 crate
- R1 实施修复（8 个 commit）：
  - `span`: `split_at`/`contiguous_with` trait bound 放宽至 `Copy + Ord`
  - `span`: `SpanIter` 实现 `ExactSizeIterator` + `size_hint`
  - `memory_types`: 地址算术运算（`Add`/`Sub`）后校验结果在有效范围内（`Self::new()`）
  - `memory_types`: `phys_to_virt`/`virt_to_phys` 自由函数改为方法 `PhysAddr::to_virt()`/`VirtAddr::to_phys()`，启用校验（不再绕过 `new()`）
  - `memory_types`: `#[allow(clippy::...)]` 改为 `#[expect(..., reason = "...")]`
  - `build_common`: 补充逐文件 `rerun-if-changed`
  - `span`、`build_common`: 添加 README
  - `memory_types`、`config`: 更新 README 反映方法式 API

### 关键决策

| # | 决策 | 状态 |
|---|------|------|
| 地址算术校验 | 方案 A：`Add`/`Sub` 结果通过 `Self::new()` 校验 | 已实施 |
| 地址转换风格 | 方法式 `pa.to_virt()` / `va.to_phys()`（Theseus 风格） | 已实施 |
| 转换校验 | 移除为测试环境设计的绕过，`to_virt()`/`to_phys()` 调用 `new()` | 已实施 |
| `config` 依赖 `log` | 方案 A：保持现状（`log` 轻量，仅用 `LevelFilter` 类型） | 已决定 |
| `Span::merge` | 仅支持相邻区间，重叠视为错误 | 已决定 |
| `span` 独立发布 | 可独立发布到 crates.io，当前无紧迫需求 | 记录 |

### 未决设计问题

无

### R8 待办（审计收尾阶段）

- [ ] `CONTRIBUTING.md` — 贡献指南
- [ ] `CODE_OF_CONDUCT.md` — 社区行为准则
- [ ] `SECURITY.md` — 安全漏洞报告流程

## 已完成的目标

| 日期 | Phase | 内容 |
|------|-------|------|
| 2026-04-03 | R0 | 审查报告 + 基础设施实施（CI/文档/审计基线/依赖/ADR） |
| 2026-04-03 | R1 | 审查报告 + 实施修复（span/config/build_common/memory_types） |
