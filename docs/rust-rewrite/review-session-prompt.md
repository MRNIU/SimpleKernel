# SimpleKernel 审计 Session Prompt

> **使用方法**：每次新对话时，将下方 prompt 发送给 Claude。根据当前排查进度，替换 `{{PHASE}}` 和 `{{TARGET}}` 占位符。

---

## Prompt 模板

```
你正在帮助我对 SimpleKernel 进行深度审计与重构。

## 背景

SimpleKernel 是一个教学/学习用的 Rust no_std 内核，采用单地址空间（SAS）架构，
支持 riscv64 和 aarch64。项目从 C++ 迁移而来，需要自底向上排查每个组件，
将 C++ 遗留范式转换为地道 Rust，同时完善文档、测试和工具链。

## 核心文件

- **审计 Roadmap**: `docs/rust-rewrite/review-roadmap.md` — 全局计划和进度
- **设计总纲**: `docs/rust-rewrite/00-概述.md` — 所有设计决策
- **SAS 架构**: `docs/rust-rewrite/SAS-架构设计.md`
- **项目指令**: `CLAUDE.md`（= `AGENTS.md`）

## 当前任务

**Phase**: {{PHASE}}（如：R2 — 同步与 Per-CPU）
**排查目标**: {{TARGET}}（如：`crates/sync/`）

## 排查流程

对目标组件执行以下步骤：

### 1. 阅读与理解
- 读 trait 定义和 doc comment
- 读所有 impl 块
- 读所有 unsafe 块及其 SAFETY 注释
- 读现有测试
- 读 README（如有）

### 2. Rust 范式审查
逐项检查：
- **所有权**：资源是否有明确的唯一所有者？是否有不必要的 `Arc` / `Clone`？
- **生命周期**：是否有不必要的 `'static`？借用是否最小化？
- **Typestate**：状态机能否编码到类型系统中？（参考 frame_allocator 的做法）
- **RAII**：资源获取/释放是否通过 Drop 自动管理？有无 `mem::forget` 泄漏？
- **零成本抽象**：泛型 vs `dyn Trait` 的选择是否合理？
- **错误处理**：`Result` / `?` 使用是否一致？错误类型是否有信息量？
- **Unsafe 最小化**：每个 unsafe 块能否用安全抽象替代？SAFETY 注释是否充分？
- **可见性**：`pub` 接口是否最小化？是否有应该是 `pub(crate)` 的 `pub`？

### 3. 接口契约审查
- trait 方法的语义是否清晰？
- `# Safety` / `# Errors` / `# Panics` 文档是否完整？
- 与参考内核（Linux / Theseus / Redox / µFork）的接口对比

### 4. 输出

对每个排查目标，输出：
1. **审查报告**：发现的问题、改进建议、优先级（高/中/低）
2. **代码修改**：实施改进（先改测试能覆盖的部分，再改需要新测试的部分）
3. **文档**：
   - 模块 README（概述 / 设计决策 / 依赖图 / API / 测试说明）
   - 生命周期图（Mermaid，如类型有复杂状态转换）
   - 时序图（Mermaid，如有多组件交互流程）
4. **测试**：补充缺失的单元测试 / 系统测试

## 参考内核使用指南

| 内核 | 何时参考 | 参考什么 |
|------|----------|----------|
| **Linux** | 子系统接口设计 | VFS ops, sched_class, mm_struct, 信号, lockdep |
| **Theseus** | Rust 类型系统利用 | MappedPages, typestate, crate 隔离, DeadlockPrevention |
| **Redox** | API 设计 + 错误处理 | syscall crate, scheme VFS, Error 统一 |
| **µFork** | POSIX 兼容策略 | 哪些 POSIX 语义保留/重新诠释, capability-FD 映射 |

本地有 Theseus 源码可参考：`ref/Theseus/`

## 约束

- 所有注释和文档使用中文（`// SAFETY:` 前缀保留英文）
- Commit 格式: `<type>(<scope>): <subject>`，必须 `--signoff`
- 不引入新的 `.unwrap()`——使用 `.expect("reason")` 或 `?`
- 每个 `unsafe` 块必须有 `// SAFETY:` 注释
- 不添加 ASCII-art 分隔线注释
- 每个测试函数必须有 `///` 文档注释

## 进度跟踪

完成排查后，更新 `docs/rust-rewrite/review-roadmap.md` 中对应 Phase 的 checklist。
如果排查中发现了需要调整 roadmap 的情况（如需要拆分/合并 phase），直接在文档中记录。
```

---

## 占位符参考

| Phase | Target 示例 |
|-------|-------------|
| R0 | CI pipeline, deny.toml, unsafe audit |
| R1 | `crates/memory_types/`, `crates/config/`, `crates/span/` |
| R2 | `crates/sync/`, `crates/interrupt_state/`, `crates/per_cpu/`, `crates/macros/` |
| R3 | `crates/frame_allocator/`, `crates/paging/`, `crates/memory/` |
| R4 | `src/arch/`, `src/boot.rs`, `src/main.rs` |
| R5 | `src/task/`, `src/preempt.rs` |
| R6 | `src/device/`, `src/fs/` |
| R7 | `src/syscall/`, 可见性审计 |
| R8 | 文档/CI 重写, 项目重组 |

---

## 使用示例

```
你正在帮助我对 SimpleKernel 进行深度审计与重构。

## 背景
（同上）

## 当前任务

**Phase**: R2 — 同步与 Per-CPU
**排查目标**: `crates/sync/`

（以下排查流程同上）
```

每完成一个目标，换到下一个目标继续。同一 Phase 内的多个目标可以在一次对话中完成，
也可以分多次对话。跨 Phase 时建议开新对话以保持上下文清晰。
