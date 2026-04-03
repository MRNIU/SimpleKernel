//! 内核编译期配置常量。
//!
//! 所有可调参数集中在此 crate，其他模块通过 `config::XXX` 引用。
//! 修改常量后重编译即可生效，无需逐文件搜索魔数。

#![cfg_attr(not(test), no_std)]

/// 最大 CPU 核心数
pub const MAX_CORE_COUNT: usize = 4;

/// 页大小
pub const PAGE_SIZE: usize = 4096;

/// log2(PAGE_SIZE)
pub const PAGE_SIZE_BITS: usize = PAGE_SIZE.trailing_zeros() as usize;

/// PTE 大小位移量——log2(sizeof(u64)) = 3
const PTE_SIZE_BITS: usize = 3;

/// 单级页表索引位宽——log2(PAGE_SIZE / sizeof(PTE))
///
/// 4KB 页 + 8B PTE = 512 条目 = 9 位索引
const INDEX_BITS: usize = PAGE_SIZE_BITS - PTE_SIZE_BITS;

/// 虚拟地址有效位宽——由 [`PT_LEVELS`] 自动推导。
///
/// 计算公式：`PAGE_SIZE_BITS + PT_LEVELS × INDEX_BITS`
/// - Sv39 (PT_LEVELS=3): 12 + 3×9 = 39
/// - Sv48 / AArch64 4KB (PT_LEVELS=4): 12 + 4×9 = 48
/// - Sv57 (PT_LEVELS=5): 12 + 5×9 = 57
pub const VA_BITS: usize = PAGE_SIZE_BITS + PT_LEVELS * INDEX_BITS;

/// 物理地址有效位宽（架构固有，无法从 PT_LEVELS 推导）。
///
/// - RISC-V Sv39/Sv48/Sv57: 56 位
/// - AArch64: 48 位（LPA2 扩展到 52 位，暂不支持）
#[cfg(target_arch = "riscv64")]
pub const PA_BITS: usize = 56;
/// 物理地址有效位宽
#[cfg(target_arch = "aarch64")]
pub const PA_BITS: usize = 48;
/// 物理地址有效位宽（宿主机编译占位）
#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
pub const PA_BITS: usize = 48;

/// TLB 全局刷新阈值（页数）。
///
/// unmap 页数超过此阈值时使用全局 TLB flush，否则逐页 flush。
/// 默认 33，与常见内核实现一致。
pub const TLB_FLUSH_THRESHOLD: usize = 33;

/// 内核线程栈大小
pub const KERNEL_STACK_SIZE: usize = 4 * PAGE_SIZE;

/// 内核堆大小
pub const KERNEL_HEAP_SIZE: usize = 1024 * PAGE_SIZE;

/// Per-CPU 区域对齐
pub const PER_CPU_ALIGN_SIZE: usize = 128;

/// Per-CPU 区域最大大小
pub const PERCPU_AREA_MAX: usize = PAGE_SIZE;

/// 页表层级数（RISC-V Sv39: 3 级）
#[cfg(target_arch = "riscv64")]
pub const PT_LEVELS: usize = 3;
/// 页表层级数（AArch64 4KB granule: 4 级）
#[cfg(target_arch = "aarch64")]
pub const PT_LEVELS: usize = 4;
/// 页表层级数（宿主机编译占位）
#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
pub const PT_LEVELS: usize = 3;

/// Round-Robin 默认时间片（tick 数）
pub const SCHED_RR_TIME_QUANTUM: u64 = 5;

/// 内核 tick 频率（10 Hz = 每 100ms 一次 tick 中断）
pub const TIMER_FREQ_HZ: u64 = 10;

/// Per-CPU 锁顺序栈最大深度
pub const LOCK_STACK_DEPTH: usize = 16;

/// 自旋锁超时阈值（循环次数）——超过此值 panic。
///
/// 帮助定位死锁：超时时 panic 信息包含锁名称、owner 核心等诊断数据。
/// 仅在 `spin-timeout` feature 启用时生效。
pub const SPINLOCK_TIMEOUT: u64 = 100_000_000;

/// 内核默认日志级别
pub const DEFAULT_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

/// 回溯最大帧数
pub const MAX_BACKTRACE_DEPTH: usize = 16;

/// 日志消息体缓冲区大小（字节，栈上 heapless::String）
pub const LOG_MSG_BUF_SIZE: usize = 256;

/// 日志头部缓冲区大小（字节，栈上 heapless::String）
pub const LOG_HDR_BUF_SIZE: usize = 128;

/// Panic 格式化缓冲区大小（字节，栈上 heapless::String）
pub const PANIC_BUF_SIZE: usize = 256;

/// map 分块大小（页数）。
///
/// `MappedPages::map_alloc` 每次在栈上分配并映射的最大页数。
/// 栈消耗：`MAP_CHUNK_SIZE × size_of::<AllocatedFrames>()` ≈ `MAP_CHUNK_SIZE × 16` 字节。
pub const MAP_CHUNK_SIZE: usize = 128;

/// unmap 分块大小（页数）。
///
/// `MappedPages::unmap_and_reclaim` 每次在栈上处理的最大页数。
/// 栈消耗：`UNMAP_CHUNK_SIZE × size_of::<PhysAddr>()` ≈ `UNMAP_CHUNK_SIZE × 8` 字节。
pub const UNMAP_CHUNK_SIZE: usize = 256;

/// 物理地址到虚拟地址的固定偏移量。
///
/// - `0`：identity mapping（VA == PA），当前使用
/// - 非零值：higher-half kernel（VA = PA + PHYS_OFFSET）
pub const PHYS_OFFSET: usize = 0;

const _: () = assert!(VA_BITS > 0 && VA_BITS <= 64, "VA_BITS must be in (0, 64]");
const _: () = assert!(PA_BITS > 0 && PA_BITS <= 64, "PA_BITS must be in (0, 64]");
const _: () = assert!(
    PAGE_SIZE.is_power_of_two(),
    "PAGE_SIZE must be a power of two"
);
const _: () = assert!(
    KERNEL_STACK_SIZE.is_power_of_two(),
    "KERNEL_STACK_SIZE must be a power of two (boot.rs uses shift)"
);
const _: () = assert!(
    KERNEL_STACK_SIZE >= PAGE_SIZE,
    "KERNEL_STACK_SIZE must be >= PAGE_SIZE"
);
const _: () = assert!(MAX_CORE_COUNT > 0, "MAX_CORE_COUNT must be > 0");
const _: () = assert!(MAP_CHUNK_SIZE > 0, "MAP_CHUNK_SIZE must be > 0");
const _: () = assert!(UNMAP_CHUNK_SIZE > 0, "UNMAP_CHUNK_SIZE must be > 0");
// map: AllocatedFrames ≈ 16B，unmap: PhysAddr ≈ 8B，栈预算上限 4KB
const _: () = assert!(
    MAP_CHUNK_SIZE * 16 <= 4096,
    "MAP_CHUNK_SIZE 栈消耗超过 4KB 预算"
);
const _: () = assert!(
    UNMAP_CHUNK_SIZE * 8 <= 4096,
    "UNMAP_CHUNK_SIZE 栈消耗超过 4KB 预算"
);
