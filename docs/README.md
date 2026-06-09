<!-- Copyright The SimpleKernel Contributors -->

# docs/

本目录保存 SimpleKernel 的项目文档。代码是实现真值源；当历史设计文档与代码冲突时，以当前代码为准，并在审查记录或 ADR 中标出差异。

## 目录结构

```text
docs/
  README.md
  conventions.md
  git.md
  design/             # 当前设计说明与历史阶段设计，部分内容可能早于实现
  adr/          # ADR：架构决策记录
  audit/              # 当前深度审计计划、进度与输出格式
  diagrams/           # 可审查的架构图和依赖图
  rfcs/               # 决策前的设计空间讨论
  specs/              # 功能或子系统设计输入
  plans/              # 执行计划和验证步骤
  templates/          # 可复制模板
```

## 文档类型指南

| 需求 | 应写文档 |
|------|----------|
| 说明当前系统架构、运行时模型、边界、质量属性 | `docs/design/` 中的 SAD/架构文档 |
| 说明当前子系统设计、接口、状态机、算法、错误处理、并发模型 | `docs/design/` 中的 SDD/设计文档 |
| 记录已经做出的架构决策 | `docs/adr/` |
| 在决策前比较较大的设计方向 | `docs/rfcs/` |
| 描述功能或子系统的设计输入 | `docs/specs/` |
| 跟踪实施步骤和验证 | `docs/plans/` |
| 跟踪审计阶段目标、进度和发现 | `docs/audit/` |
| 记录 QEMU、固件链路、目标平台或外部交付物边界 | `docs/design/`、`docs/adr/` 或 `docs/audit/` |
| 创建局部协作规则或新文档 | `docs/templates/` |
| 记录长期工程约定，包括 Copyright、注释、文件规模、严格 JSON、第三方代码和运行时配置 | `docs/conventions.md` |
| 记录 Git、commit、DCO 和 `.gitmessage` 规范 | `docs/git.md` |

## SAD、SDD 与历史文档边界

- SAD 描述“当前架构是什么”，应引用 ADR 解释关键决策来源。
- SDD 描述“当前设计如何落地”，重点是接口、数据结构、状态机、算法、错误处理、并发和验证。
- ADR 记录“为什么做出某个决策”，不替代 SAD/SDD 的当前状态说明。
- RFC 记录决策前的方案探索；接受后应拆出 ADR，并同步当前设计文档。
- Spec 记录设计输入；设计稳定后应同步到 SDD 或在 SDD 中链接。
- Plan 记录执行步骤、任务拆分和验证，不替代设计文档。

SimpleKernel 当前已有 `docs/design/` 和 `docs/adr/`。新增 SAD/SDD 时可以使用 `docs/templates/sad.md`、`docs/templates/sdd.md`，也可以按子系统放入 `docs/design/`，但必须在相关入口文档中链接。

## 当前设计入口

- `docs/design/SAS-架构设计.md`：单地址空间 SAS 架构边界。
- `docs/design/memory-subsystem-v2.md`：当前内存子系统设计。
- `docs/design/device-subsystem-current.md`：当前设备子系统设计和 rdrive 决策边界。
- `docs/design/R4-arch-boot-sequence.md`：R4 启动与 SMP 上线时序。
- `docs/design/R4-interrupt-timer-flow.md`：R4 中断、timer 与 TLB shootdown 流程。
- `docs/design/R4-architecture-porting-guide.md`：新增架构后端指南。

## 图表规则

架构图、数据流、状态机、启动流程和目标平台拓扑优先使用 Mermaid 或 PlantUML。图表必须配套文字说明，不能只提交图片或截图。
