<!-- Copyright The SimpleKernel Contributors -->

# 文档模板

本目录保存可复制模板。使用时复制到目标目录并改名，不要直接在模板文件中填写真实项目内容。

| 模板 | 用途 | 建议目标位置 |
|------|------|--------------|
| `sad.md` | 当前软件架构、边界、运行时视图、质量属性 | `docs/design/sad-*.md` 或 `docs/SAD.md` |
| `sdd.md` | 当前系统或子系统设计、接口、状态机、算法 | `docs/design/sdd-*.md` 或 `docs/SDD.md` |
| `rfc.md` | 决策前的方案探索 | `docs/rfcs/YYYY-MM-DD-topic.md` |
| `spec.md` | 功能或子系统设计输入 | `docs/specs/YYYY-MM-DD-topic.md` |
| `plan.md` | 执行计划、任务拆分和验证 | `docs/plans/YYYY-MM-DD-topic.md` |
| `local-AGENTS.md` | 模块或 crate 的局部协作规则 | 模块根目录 `AGENTS.md` |
| `adr-template.md` | 架构决策记录 | `docs/adr/NNN-title.md` |
| `module-readme-template.md` | crate 或模块 README | 模块目录 `README.md` |

图表优先使用 Mermaid 或 PlantUML。图表必须配套文字说明，不能替代接口、参数、验收标准和责任边界。
