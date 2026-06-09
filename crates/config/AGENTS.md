<!-- Copyright The SimpleKernel Contributors -->

# config

内核编译期配置常量——所有可调参数集中在此，消除魔数散布。

## 概览

`config` 将内核的编译期常量集中管理，其他 crate 通过 `config::XXX` 引用。
修改常量后重编译即可生效，无需逐文件搜索魔数。

本 crate 无 `alloc` 依赖，唯一外部依赖是 `log`（仅用于 `DEFAULT_LOG_LEVEL` 的类型）。

## 常量一览

### 内存

| 常量 | 默认值 | 说明 |
|------|--------|------|
| `PAGE_SIZE` | 4096 (4KB) | 页大小 |
| `PAGE_SIZE_BITS` | 12 | log2(PAGE_SIZE)，由 `trailing_zeros()` 自动计算 |
| `KERNEL_STACK_SIZE` | 16KB | 内核线程栈大小（必须为 2 的幂且 >= PAGE_SIZE） |
| `KERNEL_HEAP_SIZE` | 4MB | 内核堆大小 |

### 页表

| 常量 | 默认值 | 说明 |
|------|--------|------|
| `PT_LEVELS` | 3 (riscv64) / 4 (aarch64) | 页表层级数，按架构 `cfg` 选择 |
| `TLB_FLUSH_THRESHOLD` | 33 | 超过此页数时用全局 TLB flush |

### SMP / Per-CPU

| 常量 | 默认值 | 说明 |
|------|--------|------|
| `MAX_CORE_COUNT` | 4 | 最大 CPU 核心数 |
| `PER_CPU_ALIGN_SIZE` | 128 | Per-CPU 区域对齐（避免 false sharing） |
| `PERCPU_AREA_MAX` | 4KB | Per-CPU 区域最大大小 |
| `LOCK_STACK_DEPTH` | 16 | Per-CPU 锁顺序栈最大深度 |

### 调度与定时

| 常量 | 默认值 | 说明 |
|------|--------|------|
| `SCHED_RR_TIME_QUANTUM` | 5 tick | Round-Robin 默认时间片 |
| `TIMER_FREQ_HZ` | 10 Hz | tick 中断频率（每 100ms 一次） |

### 调试

| 常量 | 默认值 | 说明 |
|------|--------|------|
| `DEFAULT_LOG_LEVEL` | Debug | 内核日志级别 |
| `MAX_BACKTRACE_DEPTH` | 16 | 回溯最大帧数 |

## 编译期校验

crate 底部通过 `const _: () = assert!(...)` 在编译期检查不变量：

- `PAGE_SIZE` 必须为 2 的幂
- `KERNEL_STACK_SIZE` 必须为 2 的幂且 >= `PAGE_SIZE`
- `MAX_CORE_COUNT` 必须 > 0

违反任一条件会导致编译错误，而非运行时 panic。

## 架构相关常量

`PT_LEVELS` 按 `target_arch` 选择：

| 架构 | 值 | 对应模式 |
|------|----|----------|
| `riscv64` | 3 | Sv39 |
| `aarch64` | 4 | 4KB granule |
| 其他（宿主机测试） | 3 | 回退到 Sv39 |

## 注意事项

### 1. `KERNEL_STACK_SIZE` 必须为 2 的幂

`boot.rs` 中使用移位运算计算栈偏移，依赖此不变量。编译期 assert 已保证。
