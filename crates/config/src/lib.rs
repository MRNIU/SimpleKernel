#![cfg_attr(not(test), no_std)]

/// 最大 CPU 核心数
pub const MAX_CORE_COUNT: usize = 4;

/// 页大小
pub const PAGE_SIZE: usize = 4096;

/// log2(PAGE_SIZE)
pub const PAGE_SIZE_BITS: usize = PAGE_SIZE.trailing_zeros() as usize;

/// 内核线程栈大小
pub const KERNEL_STACK_SIZE: usize = 4 * PAGE_SIZE;

/// Per-CPU 区域对齐
pub const PER_CPU_ALIGN_SIZE: usize = 128;

/// 4 MB，由 BSS 段静态数组支撑
pub const KERNEL_HEAP_SIZE: usize = 1024 * PAGE_SIZE;

/// 内核默认日志级别
pub const DEFAULT_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

/// 回溯最大帧数
pub const MAX_BACKTRACE_DEPTH: usize = 16;

/// 10 Hz = 每 100ms 一次 tick 中断
pub const TIMER_FREQ_HZ: u64 = 10;

/// Per-CPU 锁顺序栈最大深度
pub const LOCK_STACK_DEPTH: usize = 16;

/// percpu 区域最大大小
pub const PERCPU_AREA_MAX: usize = PAGE_SIZE;

/// 页表层级数
///
/// - RISC-V Sv39: 3 级
/// - AArch64 4KB granule: 4 级
#[cfg(target_arch = "riscv64")]
pub const PT_LEVELS: usize = 3;
#[cfg(target_arch = "aarch64")]
pub const PT_LEVELS: usize = 4;
/// 宿主机编译占位（测试 / clippy）
#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
pub const PT_LEVELS: usize = 3;
