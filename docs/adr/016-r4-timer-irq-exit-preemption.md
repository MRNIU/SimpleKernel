<!-- Copyright The SimpleKernel Contributors -->

# ADR-016: R4 timer IRQ-exit 抢占边界

## 状态

**提议**

## 日期

2026-05-09

## 审计阶段

R4 — 架构层

## 涉及模块

`src/timer.rs`、`src/task/sched.rs`、`src/preempt.rs`、`src/arch/riscv64/interrupt.rs`、
`src/arch/aarch64/interrupt.rs`

## 背景

R4-02 发现 timer tick 到调度器之间的闭环不完整：tick handler 过去只设置 `need_resched`，
实际调度依赖主循环或其他安全点；同时公共 timer 层曾无条件置位，绕过 scheduler policy 的
`timer_tick()` 返回值。

SimpleKernel 需要保留一个硬边界：不能在 hard IRQ 计数仍然打开时直接执行 `schedule()`，否则调度锁、
中断状态和上下文切换之间容易形成不可审计的嵌套。

## 备选方案

### 方案 A：IRQ handler 退出 hardirq 后执行 post-IRQ preempt hook

timer 层只根据 scheduler policy 返回值设置 pending reschedule；架构 IRQ handler 在
`HardIrqGuard` 释放后调用 `task::preempt_after_irq()`，由它消费一次 pending flag 并进入 `schedule()`。

**优点**:
- 保留“硬中断内不 schedule”的边界。
- timer-driven preemption 可以闭环，不依赖任务主动 yield。
- FIFO/RR/CFS 等策略的 tick 结果成为唯一抢占来源。

**缺点**:
- 每个架构 IRQ handler 都必须在正确位置调用 post-IRQ hook。
- 当前仍是每核 pending flag，不包含更复杂的抢占原因和优先级。

### 方案 B：只保留协作式 reschedule flag

R4 只置位 `need_resched`，由主循环、syscall 或未来安全点消费，文档明确当前不是 timer 抢占。

**优点**:
- 实现简单。
- 不需要修改 trap 返回路径。

**缺点**:
- busy loop 任务可能长期不让出 CPU。
- scheduler policy 的时间片语义不能真正生效。

### 方案 C：timer tick 完全下沉给 scheduler

架构层只负责硬件 ack/rearm，task scheduler 直接处理 tick、抢占和上下文切换。

**优点**:
- 调度语义集中。
- 后续可扩展更丰富的 policy 事件。

**缺点**:
- R4 架构层和 R5 task 层耦合更强。
- 当前实现需要重排较多中断路径。

## 决策

选择 **方案 A**。

timer IRQ 在 hardirq 处理结束后、trap 返回前消费一次 pending reschedule，由 scheduler policy 决定是否抢占。

## 理由

方案 A 是当前 R4 范围内最小的语义闭环：它不把 `schedule()` 放进 hard IRQ 内部，但也不让 timer tick
退化成只有主循环才能消费的协作式提示。对 C/C++ 背景可以类比为“中断下半段的固定抢占检查点”：
硬件中断处理已经结束，才进入会切换栈和任务状态的调度路径。

这个选择依赖每个架构的 trap/IRQ handler 都遵守同一调用位置：必须在 `HardIrqGuard` 释放后调用
`task::preempt_after_irq()`。新增架构时需要在移植指南和 arch-test 中覆盖该边界。

## 影响

- **代码变更**: `timer::handle_timer_common()` 只在 `task::timer_tick()` 返回 true 时设置 pending flag；
  RISC-V/AArch64 IRQ handler 在 hardirq guard 退出后调用 `task::preempt_after_irq()`。
- **API 变更**: 增加 `preempt::take_irq_exit_preemption_request()` 作为一次性消费入口。
- **测试**: `arch-test` 覆盖 pending flag 在 IRQ-exit 判定中只消费一次。
- **文档**: R4 中断/timer 流程文档记录 post-IRQ preempt hook。
- **当前设计同步**: R5 scheduler 若调整 policy，需要保持 `timer_tick()` 返回值仍是抢占唯一来源。

## 参考

- `docs/audit/2026-05-08-r4-architecture-review-findings.md` — R4-02
- `docs/design/R4-interrupt-timer-flow.md` — 当前 timer 与 IRQ-exit 流程
