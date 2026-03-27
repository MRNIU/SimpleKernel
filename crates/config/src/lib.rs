#![cfg_attr(not(test), no_std)]

pub const MAX_CORE_COUNT: usize = 4;

/// 页大小，单位字节
pub const PAGE_SIZE: usize = 4096;

/// 页大小的位数（log2(PAGE_SIZE)）
pub const PAGE_SIZE_BITS: usize = 12;

/// Kernel heap size: 4 MB (backed by static BSS array)
pub const KERNEL_HEAP_SIZE: usize = 4 * 1024 * 1024;

/// 内核日志级别（可按需调整为 Trace/Info/Warn/Error）
pub const DEFAULT_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

/// 回溯最大深度
pub const MAX_BACKTRACE_DEPTH: usize = 16;

/// 内核线程栈大小（16KB，与 boot.S 中 DEFAULT_STACK_SIZE 一致）
pub const KERNEL_STACK_SIZE: usize = 16 * 1024;

/// 内核 tick 频率（Hz）——所有架构统一使用此值。
///
/// 100 Hz = 每 10ms 一次中断，适合调试和测试。
pub const TIMER_FREQ_HZ: u64 = 100;

/// 页表层级数——由目标架构决定。
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
