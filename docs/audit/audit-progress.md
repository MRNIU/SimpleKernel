# 审计进度

> 此文件由 AI 在每次审计对话结束时自动更新，用于跨对话传递上下文。
> 请勿手动编辑，除非需要纠正 AI 的记录。

## 当前状态

**当前 Phase**: R0 — 基线建立（实施中）
**下一个目标**: R1 — 原语层

## 上次对话摘要

**日期**：2026-04-03

### 已完成

- R0.1 CI 审计：识别出重复 step 块、缺少 matrix、缺少单元测试/cargo-deny/unsafe 统计
- R0.2 文档基础设施：确认现有 crate README 覆盖率高（10/18），缺模块 README 模板
- R0.3 Unsafe 审计基线：142 unsafe 块，SAFETY 注释覆盖率 ~63%，缺口集中在 paging/memory
- R0.4 依赖审计：2 个 git 依赖（aarch64-cpu fork、fatfs），65 个外部依赖，许可证全部兼容
- R0.5 分支策略：243 commits 领先 main，无冲突，决定审计完成后 merge --no-ff

### R0 实施产出

- [x] `deny.toml` — licenses + bans + advisories + sources
- [x] `docs/templates/module-readme-template.md` — 架构设计 + 模块细节
- [x] `docs/audit/unsafe-audit-baseline.md` — 按文件/目录的 unsafe 统计
- [x] `docs/audit/dependency-audit.md` — git 依赖、版本、许可证
- [x] `docs/diagrams/crate-dependency-graph.md` — Mermaid 自动生成
- [x] `.github/actions/setup/action.yml` — composite action（checkout + submodule + rustup + cache）
- [x] `.github/workflows/workflow.yml` — 重构：composite action + matrix + cargo-deny + 单元测试 + 修复 Clippy target
- [x] `.github/workflows/docs.yml` — Rustdoc GitHub Pages 部署
- [x] `docs/decisions/001-aarch64-float-support.md` — ADR: AArch64 硬件浮点（提议）

### 关键决策

| # | 决策 | 状态 |
|---|------|------|
| CI 重构 | composite action + matrix | 已实施 |
| unsafe 统计 | cargo-geiger（Rust 官方工具链） | 已决定，待 CI 集成 |
| 系统测试重复次数 | PR:3 / push:10 保留，待进一步讨论 | 暂缓 |
| AArch64 target | 当前统一为 softfloat，R4 切换到硬件浮点 | ADR-001 提议 |
| 分支合并 | merge --no-ff，审计全部完成后执行 | 已决定 |
| `aarch64-cpu` | 暂用 fork，上游 PR 合入后切回 | 已决定 |
| `fatfs` | 更新到最新，继续用 git 依赖 | 已决定 |
| Rustdoc | 发布到 GitHub Pages | 已实施 |

### 未决设计问题

- 系统测试重复次数策略（#3）需进一步讨论

### R8 待办（审计收尾阶段）

- [ ] `CONTRIBUTING.md` — 贡献指南
- [ ] `CODE_OF_CONDUCT.md` — 社区行为准则
- [ ] `SECURITY.md` — 安全漏洞报告流程

## 已完成的目标

| 日期 | Phase | 内容 |
|------|-------|------|
| 2026-04-03 | R0 | 审查报告 + 基础设施实施（CI/文档/审计基线/依赖/ADR） |
