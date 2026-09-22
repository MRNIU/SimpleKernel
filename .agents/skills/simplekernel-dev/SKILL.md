---
name: simplekernel-dev
description: Use when implementing, fixing, refactoring, reviewing, documenting, or validating code in the SimpleKernel repository. 仅用于 SimpleKernel 开发任务，不适用于其他仓库。
---
<!-- Copyright The SimpleKernel Contributors -->

# SimpleKernel 开发

在当前 SimpleKernel checkout 中按任务范围工作。以下链接相对本文件，命令从仓库根执行。
资料只按触发条件读取；本轮已读且未变的规则不重复加载。

## 1. 确认范围与基线

确认用户要实现、审查还是文档维护，以及明确授权与排除项。用 Git 确认仓库根、branch、HEAD、
index 和工作区，保留无关改动。读取 [根 AGENTS](../../../AGENTS.md) 与目标路径的局部规则；
需要工程细节时查 [conventions](../../../docs/conventions.md)。不因任务开始而全仓扫描。

## 2. 找到事实链

- 代码任务：从目标 trait / API 契约到实现、直接调用方和测试；用定向搜索补足所有权与失败路径。
  契约、设计或边界有疑义时，从 [docs 索引](../../../docs/AGENTS.md) 找当前设计；历史设计仅作背景。
- 普通局部审查：只读目标及支撑结论的调用链，输出位置、触发、风险、测试缺口和最小修复方向，
  然后停止等待反馈。只读要求也禁止写进度文档。
- 用户明确要求专项深度审计或继续审计：按根 AGENTS 路由到 Roadmap、当前进度与报告格式；
  不把该流程套用到普通修复、片段审查或文档改动。
- 文档任务：核验受影响内容的当前文件、符号、命令或证据来源；只读必要实现，不重做架构审计。

## 3. 判断是否需要决策

契约内的局部修复直接按已授权范围实施。若要改变 SAS/平台不变量、资源所有权、跨 crate 边界，
或在长期架构方案中取舍，按 [ADR 规则](../../../docs/adr/AGENTS.md) 明确待决点与影响，
在未决选择处停下讨论；可继续不依赖该选择的工作。AI 不替作者接受 ADR。
普通小改不强制生成计划或设计文档，不自动委派 subagent。

## 4. 实施并保护边界

SimpleKernel 是 Rust `no_std` / SAS 内核，支持 riscv64 与 aarch64；APP 唯一公共网关仍是目标。
遵循所属模块契约，检查 RAII 所有权/释放顺序、锁序、中断上下文、per-CPU、`Send`/`Sync`、
atomic ordering 与 unsafe 前提。保持不变量 fail-fast 和预期失败的类型化向上传递，
不以 fallback 隐藏非法平台输入。修改公开错误值时先判断是否改变调用方合同。
只同步实际受影响的调用方和回归测试，不顺带实现未决能力。

## 5. 选择验证

按 [贡献指南](../../../CONTRIBUTING.md#环境与命令) 使用开发者选择的本地或容器环境，
确认当前 checkout 与 `rust-toolchain.toml`；容器仅为可选依赖环境，不另造 xtask 包装脚本。
缺少依赖时在所选环境补齐；若用户另有机器级安装限制，遵循该限制。

按 [贡献指南验证表](../../../CONTRIBUTING.md#按改动选择验证) 和局部规则选择：

| 任务 | 验证选择 |
|------|----------|
| 纯逻辑 bug 修复 | package host 测试与 lint；只在影响实际平台行为时增加对应 target / QEMU |
| 裸机或跨架构变更 | 受影响 target 的检查/构建；共享代码兼顾两架构，运行相关定点系统测试 |
| 只读审查 | 静态证据优先；仅在回答具体疑点且任务允许时执行必要验证，报告未运行项 |
| 文档、链接、协作入口 | diff、受影响链接/路径/命令/规则检查；不无条件运行 Rust 或 QEMU |

host stub 不证明真实寄存器、中断、SMP 或真机 DMA；QEMU 也不证明真机 non-coherent DMA。
测试名或可用性不确定时，由 `cargo xtask test --list` 确认。命令与失败处理见
[xtask 手册](../../../xtask/AGENTS.md#超时失败与清理)：run/test 使用有界超时，默认 30 秒；
编译准备另设外层预算，debug 使用交互或外层限时。失败保留日志、定位阶段并核查/清理本次
QEMU 残留，不靠无限重试。未满足 [测试通过条件](../../../tests/AGENTS.md#通过条件与证据)
就不宣称通过。所选环境无法完成验证时报告限制，不将源码参数核验表述为执行成功。

## 6. 同步与交付

依 [贡献指南归属表](../../../CONTRIBUTING.md#文档同步) 更新真正受影响的规则、设计和入口，
不复制详细规则，不把当前 HEAD、阶段任务或测试数量写入本 skill。
检查最终 diff、`git diff --check` 和无关改动；中文报告改变了什么、原因、实际执行的命令与
结果、未验证边界和剩余事项。必要时用 C/C++ 类比解释 Rust 所有权或类型约束。
仅在用户授权时提交或 push；提交必须遵循 [.gitmessage](../../../.gitmessage)，包括 DCO。
