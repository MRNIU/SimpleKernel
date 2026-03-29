# tick

全局单调 tick 计数器——内核调度时基。

## 概览

`tick` 提供一个由 BSP（Bootstrap Processor）定时器中断驱动的全局计数器，
使 `task`、`scheduler` 等上层模块无需依赖架构层即可读取时间。

独立 crate 的原因：tick 计数是调度器和睡眠机制的基础依赖，
但它本身不需要知道定时器硬件细节——只需一个原子计数器和两个函数。
拆出来后，调度器依赖 `tick` 而非整个 `arch`，依赖方向更清晰。

## API

```rust
/// 由 BSP 的 timer handler 调用——递增并返回新值。
/// 非 BSP 核心调用时只读取当前值，不递增。
pub fn advance(is_bsp: bool) -> u64;

/// 读取当前 tick 计数。
pub fn current() -> u64;
```

只有 BSP 递增计数器，避免 N 核并发递增导致 tick 膨胀
（例如 4 核时每次定时器中断 tick 增加 4 而非 1）。

## 调用链

```
arch::timer_handler()
  └→ tick::advance(is_bsp)     ← BSP: fetch_add(1, AcqRel) + 1
                                  从核: load(Acquire)

scheduler / sleep / timeout
  └→ tick::current()           ← load(Acquire)
```

## 与 `config::TIMER_FREQ_HZ` 的关系

`TIMER_FREQ_HZ`（默认 10 Hz）决定定时器中断的触发频率，
即每秒调用 `advance()` 的次数。换算为时间：

```
经过时间 = tick::current() / config::TIMER_FREQ_HZ
```

修改 `TIMER_FREQ_HZ` 不需要改动本 crate——只影响 `arch` 层的定时器配置。

## TODO

### Per-CPU tick 计数器

当前仅维护全局计数器（类似 Linux `jiffies`），BSP 单点递增，
缺少 per-CPU tick 记账，无法精确追踪每核时间片消耗。

参考 Linux `tick_sched` 引入 per-CPU 计数器，用于：
- 调度记账（CFS vruntime 推进）
- 时间片耗尽检测
- 消除 BSP 单点故障对全局时间的影响
