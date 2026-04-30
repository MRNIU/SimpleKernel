# 项目约定

本文记录 SimpleKernel 长期协作约定。审计阶段的临时规则仍以根目录 `AGENTS.md` 的“CURRENT PHASE”部分为准；临时规则清理后，应把仍然有效的规则沉淀到本文。

## 语言与文档

- 项目文档、注释、文档注释、ADR、RFC、Spec、Plan、PR 描述和 commit subject 优先使用中文。
- 技术术语、协议名、寄存器名、指令名、crate 名、类型名、函数名、文件名、命令、配置键和环境变量保持英文或生态通用写法。
- 中文解释和英文术语同时出现时，优先使用“中文说明 + 英文术语”，例如“单地址空间 SAS”“页表项 PTE”。
- 易过期内容必须带日期，例如路线图、阶段状态、审计结论、临时兼容策略。
- 文档引用文件路径时使用仓库相对路径，不写本地绝对路径、用户名、个人机器名或个人工具配置。

## 仓库结构

```text
repo/
  AGENTS.md
  README.md
  CONTRIBUTING.md
  SECURITY.md
  CODE_OF_CONDUCT.md
  .devcontainer/
  .github/
  3rd/
  crates/
  docs/
  src/
  tests/
  xtask/
```

目录规则：

- `src/` 保存内核主体代码，`crates/` 保存 workspace 子 crate，`tests/` 保存独立 QEMU 测试二进制。
- `.devcontainer/` 是默认开发环境入口；构建、检查、pre-commit、固件构建、QEMU 和系统测试优先在容器内执行。
- `docs/adr/` 是 ADR 目录，用于保存架构决策记录。
- `docs/design/` 保存当前设计文档和历史阶段设计。历史阶段文档可能早于实现；代码和 ADR 优先级更高。
- 大模块如果有独立边界、依赖约束或修改 checklist，可在模块根目录放局部 `AGENTS.md`，从 `docs/templates/local-AGENTS.md` 复制后改写。

## 文档新鲜度

1. 代码变更导致文档失效时，必须在同一个 PR 更新文档。
2. 架构不变量变化时，必须更新 `docs/adr/` 或相关 SAD/SDD。
3. 公开 trait、错误码、启动流程、测试入口、命令参数变化时，必须更新 README、相关设计文档和测试说明。
4. 被文档引用的文件移动时，必须同步更新引用。
5. AI 工具记忆、聊天记录和本地草稿不是权威来源；写入仓库前必须回到仓库文件验证。

## SAD、SDD、ADR、RFC、Spec、Plan

| 类型 | 作用 | 位置 |
|------|------|------|
| SAD | 当前软件架构、边界、运行时/部署视图、质量属性 | `docs/design/` 或 `docs/SAD.md` |
| SDD | 当前系统或子系统设计、接口、数据模型、状态机、算法 | `docs/design/` 或 `docs/SDD.md` |
| ADR | 已做出的架构决策和理由 | `docs/adr/` |
| RFC | 决策前的设计空间探索 | `docs/rfcs/` |
| Spec | 功能或子系统设计输入 | `docs/specs/` |
| Plan | 执行步骤、任务拆分、验证和收尾状态 | `docs/plans/` |

规则：

- SAD/SDD 描述当前状态；ADR/RFC 描述历史决策和方案讨论。
- Spec 稳定后应同步到 SDD，或在 SDD 中链接。
- Plan 完成或废弃后必须写明状态，不让 checklist 长期悬空。
- AI 生成的 ADR 状态必须为“提议”；只有项目作者 review 后才可改为“已接受”。

## 图表

- 架构、启动流程、状态机、数据流、硬件拓扑、生产流程和 SOP 优先使用 Mermaid 或 PlantUML。
- 图表必须紧邻解释文字，说明边界、输入输出、失败路径和验证方式。
- 如果图表只是备选方案，标题或说明中必须标注“方案”或“草案”。

## 第三方代码与固件

`3rd/` 只放需要随仓库固定版本的第三方源码、固件、工具源码或供应商交付物。能由 Rust Cargo、系统包管理器、Dev Container Dockerfile 或 CI 安装脚本管理的依赖，不复制到 `3rd/`。

使用 `3rd/` 时必须记录：

| 字段 | 要求 |
|------|------|
| 来源 | 上游仓库、供应商、下载地址或交付批次 |
| 集成方式 | git submodule、vendor copy、下载脚本、包管理器 |
| 版本 | tag、commit SHA、release 编号、供应商版本或校验和 |
| 许可证 | LICENSE、NOTICE、商业授权或内部限制 |
| 更新方式 | 更新命令、review 要点和回滚方式 |
| 验证方式 | 构建、QEMU、硬件 bring-up、生产验收或 CI 命令 |
| 真值源 | 上游、供应商包、生成输入或仓库源码 |

当前固件相关 submodule 的初始化和验证命令应写在 README 或 `docs/production/` 中。

## 生成物

生成文件必须说明：

- 源输入：schema、IDL、设备树、脚本、供应商工具或配置。
- 生成器：命令、工具版本、运行位置和环境要求。
- 提交策略：是否提交仓库，是否由 CI 重新生成校验。
- 校验策略：diff 检查、编译检查、QEMU、硬件验证或生产验收。
- 真值源：生成物和输入冲突时，以哪个为准。

## 硬件、生产、供应商与 SOP

- 硬件接口、固件链路、设备树、生产夹具和板级 bring-up 写入 `docs/hardware/`。
- 固件构建、发布、生产测试、回滚和批次验证写入 `docs/production/` 或 `docs/sop/`。
- 外部固件、SDK、认证、外协和供应商交付物写入 `docs/suppliers/`。
- 重复执行的流程必须写成 SOP，包含前置条件、步骤、通过标准、失败处理和记录要求。

## Rust 约定

- 使用 Rust nightly、`#![no_std]`、`#![no_main]`、edition 2024。
- 公共 API 文档注释使用 `///`，涉及安全、错误或 panic 时保留 `# Safety`、`# Errors`、`# Panics` 英文节标题，内容用中文。
- 所有 `unsafe` 块必须有紧邻的 `// SAFETY:` 注释，说明调用方保证了哪些不变量。
- 不使用 `.unwrap()`；可恢复错误使用 `?`，内部不变量违反使用 `.expect("带关键数据的原因")` 或 `panic!()`。
- 内核互斥使用项目自定义 `SpinLock<T>`，不要用 `spin::Mutex` 替代中断感知锁。
