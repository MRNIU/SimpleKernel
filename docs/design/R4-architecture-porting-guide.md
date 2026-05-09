<!-- Copyright The SimpleKernel Contributors -->

# R4 新增架构指南

> 状态：当前实现指南，更新于 2026-05-09。
>
> 范围：新增一个与 `riscv64`、`aarch64` 同级的裸机架构后端。

SimpleKernel 的 R4 架构层只提供启动、console、timer、interrupt、IPI、MMU 激活和上下文切换等底层能力。
任务、设备、文件系统和 syscall 不应直接依赖架构内部模块；需要跨层能力时应通过内核 crate 内部 facade
或上层抽象暴露。

## 必须实现的文件

新增架构至少需要提供：

| 文件 | 职责 |
|------|------|
| `src/arch/<arch>/boot.S` | 入口、每核栈选择、core id 寄存器初始化、跳转 `_start` |
| `src/arch/<arch>/mod.rs` | 实现 `ArchOps` |
| `src/arch/<arch>/console.rs` | early console 输出 |
| `src/arch/<arch>/timer.rs` | 主核/从核 timer 初始化、IRQ ack/rearm |
| `src/arch/<arch>/interrupt.rs` | trap vector、IRQ 分发、timer/IPI/external interrupt |
| `src/arch/<arch>/ipi.rs` | 唤醒从核、发送 IPI、IPI handler 入口 |
| `src/arch/<arch>/mmu.rs` | 激活内核页表、必要的本地 TLB 操作 |
| `src/arch/<arch>/context.rs` | `CalleeSavedContext`、`TrapContext`、初始线程上下文 |
| `src/arch/<arch>/switch.rs` | `switch_to` ABI 实现 |
| `src/arch/<arch>/pte.rs` | PTE 属性编码和地址位契约 |

同时需要更新：

- `targets/<arch>.json`
- `xtask` 的目标枚举、QEMU 参数和 FIT/固件路径。
- `crates/arch` 中的 `PA_BITS`、`VA_BITS`、TLB、本地中断和 per-CPU 基址原语。
- `docs/design/` 中的架构启动、interrupt/timer 流程说明。

## `ArchOps` 合约

`ArchOps` 是 kernel crate 内部 trait，不对 APP crate 暴露。新增架构必须实现：

| 方法 | 要求 |
|------|------|
| `dtb_addr()` | unsafe 边界必须说明 boot args 布局和生命周期 |
| `init_timer()` / `init_timer_smp()` | 先配置硬件 timer，再由 interrupt 层打开 IRQ |
| `init_interrupt()` / `init_interrupt_smp()` | 设置 trap vector，配置 interrupt controller，最后打开 IRQ |
| `wake_secondary_cores()` | 启动失败必须 fail-fast，不能只记录 warning 后继续 |
| `send_ipi()` | 发送 doorbell 前必须发布普通内存中的 mailbox 写入 |
| `map_early_mmio()` | 只映射分页激活前必须可访问的 MMIO |
| `activate_page_table()` | unsafe 文档必须说明页表覆盖当前执行路径 |
| `console_write()` | 早期日志路径可用，不能依赖尚未初始化的设备框架 |

## 上下文与浮点策略

新增架构必须明确当前编译目标是否允许硬件浮点：

- 若允许硬件浮点，trap frame 必须保存被中断现场所需的 FP 状态，任务切换必须保存 ABI 要求的
  callee-saved FP 寄存器。
- 若禁用硬件浮点，必须在 target/features、启动状态和文档中同时声明，不能只省略上下文保存。
- RISC-V 当前按 ADR-017 采用 eager FPU：每个 hart 启用 `sstatus.FS=Dirty`，trap 保存
  `f0-f31/fcsr`，任务切换保存 `fs0-fs11`。
- AArch64 当前按 ADR-001 保存 FP/SIMD 上下文。

## 启动不变量

新增架构必须满足：

- `_boot` 为每个 CPU 设置独立启动栈。
- `_boot` 或等价入口设置 `per_cpu` 依赖的 core id 寄存器。
- core id 必须满足当前 dense `0..core_count` 平台契约；否则在 FDT topology 校验阶段 fail-fast。
- 主核先完成 `task::init()`，再打开 IRQ。
- 从核执行 `kernel_init_smp()` 后必须调用 `tlb_shootdown::mark_current_core_online()`。
- `kernel_init(Full)` 返回前，所有 FDT discovered CPU 必须 online。

## 中断与调度边界

- 硬中断 handler 必须用 `HardIrqGuard` 标记 hardirq 上下文。
- 硬中断内不能调用 `schedule()`。
- timer handler 完成硬件 ack/rearm 后调用 `timer::handle_timer_common()`。
- 如果调度策略要求抢占，由 IRQ exit 路径调用 `task::preempt_after_irq()`。
- IPI handler 需要先清除架构 pending 位，再调用具体 payload handler，避免重复触发。

## TLB 与 IPI 边界

跨核 TLB shootdown 当前使用全局 mailbox + IPI + ack generation。新增架构的 `send_ipi()` 必须保证：

- 发出 IPI 前，当前 CPU 对普通内存的 request mailbox 写入已经对目标 CPU 可见。
- RISC-V 当前使用 `fence rw, rw`。
- AArch64 当前使用 `dsb ishst` 后写 `ICC_SGI1R_EL1`。

完整 TLB shootdown 协议是否升级为 per-CPU mailbox 或 rendezvous 仍待 ADR；新增架构不应私自引入另一套协议。

## 最小验证

新增架构进入 R4 可用状态前，至少需要：

1. `cargo xtask check --arch <arch>`
2. `cargo xtask test --arch <arch> --name arch-test --timeout 30`
3. 包含以下断言的系统测试：
   - 启动栈和任务栈满足 ABI 对齐。
   - CPU topology dense 契约被验证。
   - Full 初始化返回时所有 discovered CPU 已 online。
   - timer IRQ 能进入公共 timer 层。
   - IPI 能投递到目标 CPU 并完成 ack。

涉及 QEMU 的命令必须设置 30 秒超时；超时后清理残留 `qemu-system` 进程。
