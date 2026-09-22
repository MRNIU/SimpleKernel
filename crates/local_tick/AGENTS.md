<!-- Copyright The SimpleKernel Contributors -->

# local_tick

Per-CPU tick 计数器——每核独立的调度时基。

## 概览

`local_tick` 为每个 CPU 核心维护独立的 tick 计数器，
由本核的 timer handler 驱动递增。
调度器可据此进行精确的 per-CPU 时间片记账。
按照 ADR-019 方案 B，它记录的是本核已经处理过的 timer interrupt 次数，
不补记 missed ticks，也不直接代表真实硬件 elapsed time。

与 `global_tick`（BSP 单点递增的全局计数器）互补：
- `global_tick`：全局时间推进（BSP 单点递增）
- `local_tick`：per-CPU 记账（每核独立递增）

## 边界

- 本 crate 只维护当前 CPU 的 tick 计数，不提供跨核全局时间源。
- 本 crate 不编程硬件 timer，不决定调度策略，也不补记 missed ticks。
- 全局时间推进属于 `global_tick`，timer 频率和中断触发属于架构 timer 层。

## API

```rust
/// 递增本核 tick 计数并返回新值。
/// 由本核 timer handler 在中断上下文中调用。
pub fn advance() -> u64;

/// 读取本核当前 tick 计数。
pub fn current() -> u64;
```

## 调用链

```
架构 timer IRQ handler
  → timer::handle_timer_common()
      ├→ global_tick::advance(is_bsp)   // 全局 tick
      └→ local_tick::advance()          // 本核 tick（每核独立递增）

scheduler / per-CPU accounting
  └→ local_tick::current()              // 读取本核 tick
```

## 并发安全

底层使用 `#[cpu_local]` 变量（每 CPU 独立副本），
配合 `AtomicU64` 消除同核中断嵌套的潜在竞态，
全部 API 均为 safe 函数。

## 验证入口

- 文档-only 变更：`git diff --check`。
- per-CPU tick API 或原子语义变更：`cargo clippy -p local_tick --target riscv64gc-unknown-none-elf -- -D warnings`。
- 影响 timer handler 或调度记账时，补跑相关 QEMU 测试；完整系统入口是 `cargo xtask test --arch riscv64 --timeout 30`。

## 不要假设

- 不要把本核 tick 当成全局时间源；跨核全局时间推进应使用 `global_tick`。
- 不要绕过 `#[cpu_local]` 直接共享普通可变计数器。
