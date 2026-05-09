<!-- Copyright The SimpleKernel Contributors -->

# ADR-017: RISC-V eager 浮点上下文保存

## 状态

**提议**

## 日期

2026-05-09

## 审计阶段

R4 — 架构层

## 涉及模块

`.cargo/config.toml`、`src/arch/riscv64/context.rs`、`src/arch/riscv64/interrupt.S`、
`src/arch/riscv64/interrupt.rs`、`src/arch/riscv64/switch.rs`、`tests/arch-test/`

## 背景

ADR-001 已经让 AArch64 恢复硬件浮点，并保存 FP/SIMD 上下文。R4 复审时发现 RISC-V 当前 Rust 实现只保存
整数上下文：trap path 没有保存 `f0-f31/fcsr`，任务切换没有保存 `fs0-fs11`。

Git 历史显示旧 C++/汇编实现曾保存 RISC-V 浮点状态：`TrapContext` 为 68 个 8 字节槽，
`CalleeSavedContext` 为 26 个 8 字节槽。提交 `f24ac7e8` 在 CI/结构清理中删除了该保存路径。

R4 红测确认当前代码可以执行普通 RISC-V 浮点运算，但任务切换后父任务的 `fs0` 会被子任务覆盖。
这说明问题不是“能不能执行 FP 指令”，而是“任务模型是否保存 FP 状态”。

## 备选方案

### 方案 A：禁用 RISC-V hard-float

关闭或禁止 RISC-V F/D 使用，文档声明内核和 APP 不允许依赖硬件浮点。

**优点**:
- 上下文切换成本最低。
- 汇编和状态结构简单。

**缺点**:
- 与 AArch64 hardfloat 语义不一致。
- 限制 SAS 架构下未来 APP crate 的能力。
- 当前目标三元组已经暴露 `f`/`d` target feature，禁用需要额外编译和运行期约束。

### 方案 B：eager 保存/恢复 RISC-V 浮点上下文

每个 hart 初始化时设置 `sstatus.FS=Dirty`；trap 保存 `f0-f31 + fcsr`；任务切换保存 ABI 要求的
callee-saved `fs0-fs11`。

**优点**:
- 与 AArch64 当前策略对齐。
- 恢复历史实现已有的明确布局。
- 调试简单，不依赖 FP trap 状态机。

**缺点**:
- trap path 固定增加 33 个 8 字节保存槽。
- 每次任务切换固定保存 12 个浮点寄存器，即使任务未使用 FP。

### 方案 C：lazy FPU save/restore

默认关闭 FP，首次 FP trap 时为任务分配/标记 FP 状态，按需保存恢复。

**优点**:
- 未使用 FP 的任务无需支付上下文切换成本。
- 更接近高性能通用内核常见策略。

**缺点**:
- 需要额外 trap 状态机、per-task FP owner 和多核迁移约束。
- R4 阶段会显著增加调试复杂度。

## 决策

选择 **方案 B**。

RISC-V 采用 eager FPU 策略：当前支持 hard-float，并在 trap 与任务切换路径保存必要 FP 状态。

## 理由

SimpleKernel 当前更重视架构契约清晰和测试可审计性。eager 保存恢复虽然有固定开销，但语义直接：
只要编译目标允许 FP，任务切换就不会破坏 FP callee-saved 状态；trap path 也不会丢失被中断现场。

这个选择与 ADR-001 的 AArch64 策略一致，也复用了历史实现的 68/26 槽布局。若未来性能成为瓶颈，
可以用新的 ADR 讨论 lazy FPU；在那之前，不允许只依赖“当前 workload 少用浮点”来省略上下文保存。

## 影响

- **代码变更**: `TrapContext` 扩展到 544 字节；`CalleeSavedContext` 扩展到 208 字节；
  RISC-V interrupt/switch 汇编保存恢复 FP 状态；每个 hart 初始化时设置 `sstatus.FS=Dirty`。
- **API 变更**: 无 public API 变更。
- **测试**: `arch-test` 增加 RISC-V 浮点运算和 `fs0` 跨任务切换保存回归。
- **文档**: R4 审计报告记录 R4-06 已按 eager FPU 方案修复。
- **当前设计同步**: 新架构移植时必须明确是否支持 FP 以及上下文保存策略。

## 参考

- ADR-001: `docs/adr/001-aarch64-float-support.md`
- Git 历史：`f24ac7e8` 删除旧 RISC-V 浮点上下文保存路径
- Git 历史：`72952af6` 恢复 AArch64 hardfloat 支持
- `docs/audit/2026-05-08-r4-architecture-review-findings.md` — R4-06
