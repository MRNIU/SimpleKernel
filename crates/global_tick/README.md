<!-- Copyright The SimpleKernel Contributors -->

# global_tick

全局单调 tick 计数器——内核调度时基。

## 概览

`global_tick` 提供一个由 BSP（Bootstrap Processor）定时器中断驱动的全局计数器，
使 `task`、`scheduler` 等上层模块无需依赖架构层即可读取时间。

独立 crate 的原因：tick 计数是调度器和睡眠机制的基础依赖，
但它本身不需要知道定时器硬件细节——只需一个原子计数器和两个函数。
拆出来后，调度器依赖 `global_tick` 而非整个 `arch`，依赖方向更清晰。

per-CPU tick 记账见 [`local_tick`](../local_tick/) crate。

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
  └→ global_tick::advance(is_bsp)  ← BSP: fetch_add(1, AcqRel) + 1
                                      从核: load(Acquire)

scheduler / sleep / timeout
  └→ global_tick::current()        ← load(Acquire)
```

## 与 `config::TIMER_FREQ_HZ` 的关系

`TIMER_FREQ_HZ`（默认 10 Hz）决定目标定时器中断频率。按照 ADR-019 方案 B，
`global_tick` 只表示已经处理过的 timekeeper timer interrupt 次数，不补记 missed ticks。
因此它可以作为调度和 sleep/timeout 的逻辑 tick，但不能直接当成真实硬件 elapsed time。

在没有长时间关中断、handler 延迟或 missed tick 的理想情况下，可近似换算为：

```
经过时间 = global_tick::current() / config::TIMER_FREQ_HZ
```

修改 `TIMER_FREQ_HZ` 不需要改动本 crate——只影响 `arch` 层的定时器配置。
