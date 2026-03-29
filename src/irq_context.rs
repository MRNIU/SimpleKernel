//! 中断上下文跟踪——记录当前核心是否在硬中断/软中断中。
//!
//! ## Per-CPU 状态
//!
//! | 变量 | 类型 | 说明 |
//! |------|------|------|
//! | `HARDIRQ_COUNT` | `u32` | 硬中断嵌套深度 |
//! | `SOFTIRQ_COUNT` | `u32` | 软中断嵌套深度 |

use per_cpu::cpu_local;

/// 硬中断嵌套计数（>0 表示在 hardirq 上下文中）
#[cpu_local]
static HARDIRQ_COUNT: u32 = 0;

/// 软中断嵌套计数（>0 表示在 softirq 上下文中）
#[cpu_local]
static SOFTIRQ_COUNT: u32 = 0;

/// 进入硬中断上下文（递增 hardirq_count，饱和加法防止溢出）。
///
/// 必须在中断处理程序中调用（中断已被 CPU 自动关闭）。
pub fn enter_hardirq() {
    // SAFETY: 在中断处理程序中调用，中断已关闭，无同核心并发访问
    let count = unsafe { HARDIRQ_COUNT.get_mut() };
    *count = count.saturating_add(1);
}

/// 离开硬中断上下文。
///
/// 必须与 `enter_hardirq()` 配对调用。
pub fn exit_hardirq() {
    // SAFETY: 在中断处理程序中调用，中断已关闭，无同核心并发访问
    let count = unsafe { HARDIRQ_COUNT.get_mut() };
    *count = count.saturating_sub(1);
}

/// 当前是否处于中断上下文（不可调度）。
pub fn in_interrupt() -> bool {
    *HARDIRQ_COUNT.get() > 0 || *SOFTIRQ_COUNT.get() > 0
}
