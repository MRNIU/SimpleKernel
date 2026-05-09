<!-- Copyright The SimpleKernel Contributors -->

# ADR-019: R4 timer absolute deadline 与 tick 漂移语义

## 状态

**提议**

## 日期

2026-05-09

## 审计阶段

R4 — 架构层 / R5 — 调度边界

## 涉及模块

`src/timer.rs`、`src/arch/riscv64/timer.rs`、`src/arch/aarch64/timer.rs`、
`crates/global_tick/`、`crates/local_tick/`、`src/task/sched.rs`

## 背景

R4-15 已经修复两个直接错误：

- `checked_tick_interval()` 会拒绝 `freq < TIMER_FREQ_HZ`，避免 interval 变成 0。
- RISC-V `set_timer()` 失败不再 `.ok()` 丢弃，而是带 deadline/error/value fail-fast。

剩余问题是 tick 的时间语义。当前两架构仍接近“相对重装”：

- RISC-V 在 timer interrupt 中使用 `read_time() + interval` 设置下一次 SBI timer deadline。
- AArch64 使用 `CNTV_TVAL_EL0 = interval` 重新装载相对间隔。

如果 timer handler、IPI、调度或关中断区域让本次 tick 延迟了 `delay`，下一次 deadline 会从“处理完成时的 now”
重新开始。长期看，tick 时间会向后漂移：

```text
理想 deadline: 100, 200, 300, 400
实际 handler:  100, 230, 360, 490
相对重装后:   100, 330, 460, 590
```

对教学内核来说，这可能只是统计误差；对 sleep、time slice、公平调度和超时语义来说，它会改变系统行为。

## 概念边界

需要先区分三个概念：

- **硬件 deadline**：下一次 timer interrupt 应该在哪个硬件计数值触发。
- **逻辑 tick**：内核对 scheduler、sleep queue、timeout 公开的时间单位。
- **补 tick 策略**：如果一次 handler 晚到了多个 interval，内核是否把缺失的 tick 计入逻辑时间。

相对重装只保证“handler 之后再过 interval 触发”；absolute deadline 保证“尽量贴近固定周期触发”。
但 absolute deadline 仍要决定晚到时是跳过缺失 tick，还是补记缺失 tick。

## 备选方案

### 方案 A：保留相对重装，文档声明 best-effort tick

继续使用 RISC-V `read_time() + interval` 和 AArch64 `CNTV_TVAL_EL0 = interval`。
timer tick 表示“内核实际处理过一次 timer interrupt”，不承诺贴近真实 elapsed time。

**优点**:
- 实现最简单。
- 中断 handler 固定只推进一个 tick，不会因为补 tick 在中断里做过多工作。
- 当前代码改动最小。

**缺点**:
- 长期漂移不可避免。
- sleep、timeout、scheduler vruntime 会随 handler 延迟一起变慢。
- 后续做时间相关测试时，需要接受较宽误差或只验证相对行为。

### 方案 B：absolute deadline，晚到时跳到未来但只记一个逻辑 tick

每核保存 `next_deadline`：

```text
on init:
  next_deadline = now + interval
  program(next_deadline)

on interrupt:
  now = read_counter()
  while next_deadline <= now:
      next_deadline += interval
  program(next_deadline)
  handle_timer_common(elapsed_ticks = 1)
```

RISC-V 继续使用 SBI absolute `set_timer(next_deadline)`；AArch64 改用 `CNTV_CVAL_EL0`
写绝对 deadline，而不是 `CNTV_TVAL_EL0`。

**优点**:
- 硬件触发点不会持续向后漂移。
- 每次中断只推进一个逻辑 tick，handler 成本稳定。
- 比方案 C 简单，适合先修正硬件 deadline 语义。

**缺点**:
- 如果系统晚到多个 interval，逻辑 tick 会少记；sleep/timeout 仍可能慢于真实时间。
- scheduler 不知道本次晚到了多少周期。
- 需要接受“硬件 deadline 准，但逻辑时间不补偿”的语义。

### 方案 C：absolute deadline，并补记 missed ticks

每核保存 `next_deadline`，中断时计算错过了几个周期：

```text
elapsed_ticks = 1
while next_deadline + interval <= now:
    next_deadline += interval
    elapsed_ticks += 1
next_deadline += interval
program(next_deadline)
handle_timer_common(elapsed_ticks)
```

公共 timer 层需要支持一次推进多个 tick。调度器可以选择批量记账，或只设置一次 reschedule flag。

**优点**:
- 逻辑时间更接近真实 elapsed time。
- sleep/timeout 不会因为中断延迟而无限变慢。
- 对长期运行和统计更准确。

**缺点**:
- 公共 timer、global/local tick、sleep queue 和 scheduler 需要支持批量 tick。
- 如果一次晚到很多周期，不能在中断里循环做太多调度工作，需要上限或延迟处理。
- 测试复杂度更高，需要覆盖 missed tick、批量 wakeup、调度记账边界。

### 方案 D：事件驱动 one-shot deadline

不再把 `TIMER_FREQ_HZ` 作为固定周期 heartbeat，而是根据最近的 sleep timeout、scheduler time slice
或其他事件设置下一次 one-shot deadline。

**优点**:
- 空闲时可以减少无意义 tick。
- 更接近 tickless kernel 方向。

**缺点**:
- 需要 R5/R6 层提供下一事件时间，远超 R4 修复范围。
- 当前调度器、sleep queue 和全局 tick 都按固定 tick 建模。
- 不适合作为当前阶段的直接修复。

## 决策

**暂定选择方案 B：absolute deadline，晚到时跳到未来但只记一个逻辑 tick。**

这是 R4/R5 边界上的阶段性决策：当前先消除相对重装导致的硬件 deadline 持续漂移，
不在本阶段重定义 `global_tick` / `local_tick` 的逻辑时间语义，也不补记 missed ticks。

后续如果 sleep/timeout、scheduler accounting 或 tickless one-shot 进入实现阶段，应回看本 ADR，
再决定是否从方案 B 演进到方案 C 或方案 D。

## 决策问题

本次暂定决策对原问题的回答如下：

1. `global_tick` / `local_tick` 暂时继续代表“已处理 timer interrupt 次数”，不代表硬件时间经过的 tick 数。
2. sleep/timeout 暂时不在中断延迟后追赶真实时间。
3. scheduler 的 `timer_tick()` 暂时不新增 `elapsed_ticks` 参数，只接收一次 tick / reschedule 信号。
4. timer handler 暂时不补记多个 missed ticks；handler 晚到时只把下一次硬件 deadline 推进到未来。
5. AArch64 后续实现应切到 `CNTV_CVAL_EL0` 绝对 deadline，与 RISC-V SBI absolute deadline 对齐。

## 理由

方案 B 把本次修复限制在“硬件下一次触发点”这一层：RISC-V / AArch64 都按 per-core
absolute deadline 重新装载 timer，避免每次 handler 延迟都累积到后续周期。

它刻意不解决“逻辑时间是否追赶真实时间”的问题。该问题会牵涉 `global_tick`、`local_tick`、
sleep queue、timeout 和 scheduler 记账接口，属于 R5 调度语义或更后续 tickless 设计的边界。
当前先选择方案 B，可以用较小代码面修掉硬件 deadline 漂移，并把方案 C/D 作为后续明确的回看点。

## 后续回看条件

满足以下任一条件时，应重新打开本 ADR 或补充新的 ADR：

- sleep/timeout 需要按真实 elapsed time 追赶，而不是按“已处理中断次数”推进。
- scheduler 需要知道本次 timer interrupt 晚到了多少个 interval。
- `global_tick` / `local_tick` 的语义要从“中断处理次数”改成“硬件时间 tick 数”。
- 引入 tickless one-shot deadline，固定周期 heartbeat 不再是主要 timer 模型。

## 影响

- **代码变更**:
  - 方案 A：只更新文档和测试期望。
  - 方案 B：RISC-V/AArch64 增加 per-core `next_deadline`，AArch64 使用 `CNTV_CVAL_EL0`。
  - 方案 C：在方案 B 基础上修改 `timer::handle_timer_common()`、`global_tick`、`local_tick`、
    sleep queue 和 scheduler tick 记账接口。
- **API 变更**:
  - 方案 B 可保持公共 API 基本不变。
  - 方案 C 需要引入 `elapsed_ticks` 或批量 tick API。
- **测试**:
  - 方案 B：需要 mock 或 QEMU 长时间测试，确认 deadline 不按 handler 延迟持续后移。
  - 方案 C：需要 missed tick、批量 sleep wakeup、scheduler 记账和最大补 tick 上限测试。
- **文档**: `docs/design/R4-interrupt-timer-flow.md` 需要记录最终 tick 语义。
- **当前设计同步**: 若方案 C 被接受，R5 调度设计必须同步 tick accounting 语义。

## 参考

- `docs/audit/2026-05-08-r4-architecture-review-findings.md` — R4-15
- `docs/design/R4-interrupt-timer-flow.md` — 当前 timer 与 preemption 流程
- `src/timer.rs` — 公共 tick 处理
- `src/arch/riscv64/timer.rs` — 当前 RISC-V timer rearm
- `src/arch/aarch64/timer.rs` — 当前 AArch64 timer rearm
