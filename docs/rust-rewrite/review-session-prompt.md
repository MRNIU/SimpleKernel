# SimpleKernel 审计 Session Prompt

> **使用方法**：每次新对话时，将下方 prompt 模板发送给 Claude。只需替换 `{{占位符}}`。
>
> 固定的背景信息（项目结构、编码规范、参考内核指南等）已写入 `CLAUDE.md`，
> 排查 checklist 和详细计划在 `docs/rust-rewrite/review-roadmap.md` 中，
> 此处不重复——AI 会自动读取这些文件。

---

## Prompt 模板

```
你正在帮助我对 SimpleKernel 进行深度审计与重构。

## 核心文件

- **审计 Roadmap**: `docs/rust-rewrite/review-roadmap.md`（全局计划、排查 checklist、协作流程）
- **设计总纲**: `docs/rust-rewrite/00-概述.md`
- **SAS 架构**: `docs/rust-rewrite/SAS-架构设计.md`
- **ADR 目录**: `docs/decisions/`（架构决策记录）

## 当前任务

**Phase**: {{PHASE}}（如：R2 — 同步与 Per-CPU）
**排查目标**: {{TARGET}}（如：`crates/sync/`）

## 上次对话摘要

{{LAST_SESSION_SUMMARY}}
（首次排查该目标时填"首次排查，无历史上下文"）

## 前置操作

**在开始排查前，必须先执行以下操作：**
1. Read 整个 `docs/rust-rewrite/review-roadmap.md`（定位当前 Phase 的审查范围和标准排查流程）
2. Read 排查目标的所有源文件

## 排查要求

按照 Roadmap 中"每个 Phase 的标准排查流程"执行：
1. 阅读与理解（trait → impl → unsafe → 测试 → README）
2. Rust 范式审查（所有权 / 生命周期 / typestate / RAII / 零成本 / 错误处理 / unsafe / 可见性）
3. 并发安全审查（Send/Sync 约束 / 多核竞态 / 中断重入 / 锁序 / atomic ordering）
4. 依赖与版本检查（第三方 crate 版本 / Rust nightly 特性更新 / 可替代 crate 评估）
5. 参考内核对比（与 Linux / Theseus / Redox / Zephyr / µFork 对应模块的实现思路对比）
6. 接口契约审查（doc comment 完整性）
7. 输出审查报告（见下方格式要求）

## 输出格式

请按以下格式输出审查报告：

### 模块概述
- 模块职责（一句话）
- 依赖关系（上游/下游）
- 代码量（大致行数）

### 问题清单

| # | 问题 | 严重度 | 类别 | 建议 |
|---|------|--------|------|------|
| 1 | 描述 | 高/中/低 | 范式/安全/并发/依赖/接口/文档/测试 | 改进方案 |

严重度定义：
- **高**: 正确性/安全性问题，或严重违反 Rust 惯例
- **中**: 设计可改进，影响可维护性或可扩展性
- **低**: 风格、文档、命名等非功能性改进

### 依赖与版本

| 依赖 | 当前版本 | 最新版本 | 需要操作 | 说明 |
|------|----------|----------|----------|------|
| crate 名 | x.y.z | a.b.c | 升级/无 | 变更说明 |

- 可移除的 `#![feature(...)]`（已稳定的 nightly 特性）
- 可简化代码的新 Rust 语言特性

### Crate 替代评估
当前手写的功能如果有成熟的 `no_std` crate 可替代，列出候选：

| 当前实现 | 候选 crate | 优点 | 缺点 | 备注 |
|----------|-----------|------|------|------|
| 描述 | crate 名 | ... | ... | ADR 待决 |

**不要自行替换，所有替代方案需经讨论后决定。**

### 参考内核对比

| 对比维度 | SimpleKernel | 参考内核（标注来源） | 差异原因 |
|----------|-------------|---------------------|----------|
| 描述 | 当前做法 | 其他内核做法 | SAS/教学/... |

如果参考内核有更优的设计，列入下方"设计讨论点"。

### 设计讨论点
列出需要人工决策的设计问题。对每个问题：
- 列出所有合理的备选方案（含"保持现状"选项）
- 每个方案给出客观的优缺点
- **不要推荐某个方案，不要使用"建议"、"推荐"、"最好"等词**
- 标注该决策应写入 ADR

### 建议的代码修改
按优先级排列，每项标注是否需要先补测试。

### 文档产出建议
需要产出的 README / Mermaid 图 / ADR 列表。

## 停止条件

**重要**：完成审查报告后**停下来等待我的反馈**，不要直接开始修改代码。
代码修改将在我们讨论完设计问题后进行。
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

## 对话摘要格式

每次对话结束时，让 AI 按以下格式生成摘要，下次粘贴到 `{{LAST_SESSION_SUMMARY}}`：

```
1. **已完成**: [已审查的子模块/文件列表]
2. **发现的关键问题**: [问题编号 + 一句话描述，最多 5 条]
3. **未决设计问题**: [需要继续讨论的问题]
4. **下一步**: [本次对话应从哪里继续]
```

跨 Phase 时建议开新对话以保持上下文清晰。

---

## 示例审查报告

以下是一个虚构的示例，展示期望的输出格式：

```markdown
### 模块概述
- **职责**: `crates/sync/` 提供中断安全的自旋锁原语，是所有内核并发代码的基石
- **依赖**: 上游 `interrupt_state`；下游被 `memory`, `task`, `fs` 等几乎所有模块使用
- **代码量**: ~350 行（含测试）

### 问题清单

| # | 问题 | 严重度 | 类别 | 建议 |
|---|------|--------|------|------|
| 1 | `SpinLock::force_unlock()` 无 `unsafe` 标记，但破坏了 RAII 不变量 | 高 | 安全 | 标记为 `unsafe fn`，添加 `# Safety` 文档 |
| 2 | `SpinLockGuard` 手动实现了 `Send`，但未验证 `T: Send` 约束 | 高 | 并发 | 移除手动 impl，改为 `unsafe impl<T: Send> Send for SpinLockGuard<'_, T>` 并添加 SAFETY 注释 |
| 3 | 锁级别检查仅运行时 panic，无编译期保障 | 中 | 范式 | 评估 const generic `SpinLock<T, const LEVEL: u8>` 方案（需 ADR） |
| 4 | `SpinLockGuard` 缺少 `# Safety` 节的 doc comment | 低 | 文档 | 补充 Guard 的生命周期语义说明 |

### 依赖与版本

| 依赖 | 当前版本 | 最新版本 | 需要操作 | 说明 |
|------|----------|----------|----------|------|
| `spin` | 0.9.8 | 0.10.0 | 升级 | 0.10 重构了 `Once` API，需适配调用方 |

- `#![feature(sync_unsafe_cell)]`：`SyncUnsafeCell` 已在 Rust 1.82 稳定，可移除此 feature flag

### Crate 替代评估

| 当前实现 | 候选 crate | 优点 | 缺点 | 备注 |
|----------|-----------|------|------|------|
| 自定义 `SpinLock` | [`lock_api`](https://crates.io/crates/lock_api) | 成熟、支持 `RawMutex` trait 参数化 | 不自带中断禁用语义，需自定义 `RawMutex` impl | ADR 待决 |

### 参考内核对比

| 对比维度 | SimpleKernel | 参考内核 | 差异原因 |
|----------|-------------|----------|----------|
| 锁与中断 | `SpinLock` acquire 时禁用本核中断 | [Linux: `spin_lock_irqsave`] 同样禁用中断；[Theseus: `MutexIrqSafe`] 通过 trait 参数化 | 设计一致，但 Theseus 的 trait 参数化更灵活 |
| 死锁检测 | 运行时锁级别 panic | [Linux: `lockdep`] 运行时依赖图检测 | Linux 方案更强大但复杂度高，教学项目中运行时 panic 可接受 |

### 设计讨论点
1. **编译期锁级别 vs 运行时检查**：const generic 方案可在编译期捕获锁序违规，
   但会让锁类型签名变复杂（`SpinLock<T, 3>` vs `SpinLock<T>`）。
   - (A) const generic — 优点：编译期保障；缺点：类型签名膨胀，跨模块传递复杂
   - (B) 保持运行时检查，改进 panic 信息 — 优点：简单；缺点：仅 debug 可用
   - (C) 保持现状 — 优点：无变更成本；缺点：死锁风险仍存
   → 应写入 ADR
2. **自定义 SpinLock vs `lock_api` crate**：
   - (A) 保持自定义实现 — 优点：完全控制中断语义；缺点：维护成本
   - (B) 基于 `lock_api` 实现自定义 `RawMutex` — 优点：复用成熟框架；缺点：引入依赖
   → 应写入 ADR

### 建议的代码修改
1. [高] `force_unlock()` → `unsafe fn`（无需新测试，现有测试已覆盖）
2. [高] 修正 `SpinLockGuard` 的 `Send` 约束（需先补并发测试）
3. [低] 补充 `SpinLockGuard` doc comment（纯文档变更）

### 文档产出建议
- 锁层级关系图（Mermaid）
- `crates/sync/README.md`（按模块模板）
- ADR: 编译期锁级别方案评估
- ADR: 自定义 SpinLock vs lock_api
```
