<!-- Copyright The SimpleKernel Contributors -->

# R4 中断、Timer 与 TLB Shootdown 流程

> 状态：当前实现说明，更新于 2026-05-09。
>
> 范围：`src/arch/{riscv64,aarch64}/interrupt.rs`、`src/arch/*/timer.rs`、
> `src/arch/*/ipi.rs`、`src/timer.rs`、`src/tlb_shootdown.rs`、`src/task/sched.rs`。

本文记录当前 R4 层已经落地的中断边界。RISC-V 浮点策略已由 ADR-017 收敛为 eager 保存恢复。
TLB shootdown 完整协议见 ADR-018 提议稿，仍待项目作者决策；timer absolute deadline / 长期漂移语义
见 ADR-019，已暂定采用方案 B。

## Timer IRQ 与抢占边界

```mermaid
sequenceDiagram
    participant CPU as "CPU trap entry"
    participant IRQ as "arch interrupt handler"
    participant Timer as "arch timer"
    participant Common as "timer::handle_timer_common"
    participant Task as "task scheduler"

    CPU->>IRQ: "timer interrupt"
    IRQ->>IRQ: "HardIrqGuard::enter"
    IRQ->>Timer: "ack/rearm hardware timer"
    Timer->>Common: "handle_timer_common"
    Common->>Common: "advance global/local tick"
    Common->>Task: "task::timer_tick"
    Task-->>Common: "policy says preempt?"
    Common->>Common: "set need_resched when true"
    IRQ->>IRQ: "drop HardIrqGuard"
    IRQ->>Task: "preempt_after_irq"
    Task->>Task: "schedule outside hardirq"
```

约束：

- 硬中断上下文内不直接调用 `schedule()`。
- timer 层只在 scheduler policy 返回需要抢占时设置当前核 `need_resched`。
- IRQ exit 抢占点在 `HardIrqGuard` 释放后执行，避免在 hardirq 计数非零时调度。
- `task::init()` 必须早于 IRQ enable，保证早到的 timer tick 有合法 scheduler 状态。

## TLB Shootdown 最小协议

```mermaid
sequenceDiagram
    participant Local as "initiator CPU"
    participant Mailbox as "global request mailbox"
    participant Arch as "arch IPI doorbell"
    participant Remote as "target CPU"

    Local->>Local: "flush local TLB"
    Local->>Local: "assert not hardirq and IRQ enabled"
    Local->>Mailbox: "write addr/kind/generation"
    Local->>Local: "Release fence"
    Local->>Arch: "send_ipi(target)"
    Arch->>Arch: "RISC-V fence rw,rw / AArch64 dsb ishst"
    Arch-->>Remote: "software interrupt / SGI"
    Remote->>Mailbox: "Acquire generation"
    Remote->>Remote: "flush local TLB"
    Remote->>Mailbox: "Release ack generation"
    Local->>Mailbox: "wait Acquire ack"
```

当前实现仍保留单 broadcast lock。已落地的非 ADR 硬化包括：

- 发起广播前禁止 hardirq 上下文。
- 存在远端目标时，发起广播前要求当前 CPU IRQ enabled，避免从已关中断的不可等待区域进入跨核等待。
- mailbox 发布后使用 release fence。
- RISC-V `send_ipi()` 前执行 `fence rw, rw`。
- AArch64 `ICC_SGI1R_EL1` 前执行 `dsb ishst`。

仍待项目作者决策的内容（见 ADR-018）：

- 是否继续使用单 broadcast lock，还是改为 per-CPU mailbox + sequence counter。
- 是否引入 stop-the-world rendezvous。
- 是否增加可诊断的 ack timeout 或 lock-order 证明。

## Timer Deadline 与漂移边界

当前 RISC-V / AArch64 仍按相对重装语义设置下一次 timer：

- RISC-V 使用 `read_time() + interval` 写 SBI timer deadline。
- AArch64 使用 `CNTV_TVAL_EL0 = interval`。

这会让 handler 延迟进入下一周期，长期运行时 tick 可能向后漂移。ADR-019 已暂定采用方案 B：
后续应改为 per-core absolute deadline，晚到时把下一次硬件 deadline 推进到未来，但逻辑 tick
仍只推进 1。missed tick 补记和 tickless one-shot 留到后续调度 / timeout 语义设计再回看。

## 中断分发职责

| 层 | 职责 |
|----|------|
| 汇编 trap entry | 保存/恢复 trap frame，跳转 Rust handler |
| 架构 interrupt 模块 | 识别 timer、IPI、外部中断，建立 `HardIrqGuard` |
| RISC-V FPU 上下文 | 每个 hart 启用 `sstatus.FS=Dirty`，trap 保存 `f0-f31/fcsr` |
| 架构 timer 模块 | ack/rearm 当前架构 timer，调用公共 timer 层 |
| 公共 timer 层 | 推进 tick、调用调度策略、设置 IRQ-exit 抢占标志 |
| TLB shootdown 模块 | 编码跨核请求、发送 IPI、等待远端 ack |
| task scheduler | 在 hardirq 外执行真实上下文切换 |

## 验证

- `cargo xtask test --arch riscv64 --name arch-test --timeout 30`
  - 覆盖 IRQ-exit 抢占请求只消费一次、RISC-V 浮点运算和 `fs0` 跨任务保存。
- `cargo xtask test --arch riscv64 --name tlb-shootdown --timeout 30`
  - 覆盖 online CPU 集合参与 TLB shootdown 回归。
- `cargo xtask check --arch riscv64` 和 `cargo xtask check --arch aarch64`
  - 覆盖两架构 IPI barrier 代码可编译。
