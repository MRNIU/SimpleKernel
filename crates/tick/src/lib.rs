#![cfg_attr(not(test), no_std)]

//! 全局 tick 计数器——内核调度时基。
//!
//! 提供单调递增的 tick 计数，由 BSP（core 0）的 timer handler 驱动。
//! 独立于具体的定时器硬件，
//! 使 task/scheduler 等模块无需依赖架构层即可读取时间。
//!
//! 参考 Theseus 的 `time` crate 和 Linux 的 `jiffies` 设计：
//! 将时间管理与中断控制分离，各自职责单一。

use core::sync::atomic::{AtomicU64, Ordering};

/// 全局 tick 计数器——由 BSP 的 timer handler 递增。
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// 递增 tick 计数器并返回新值——仅由 BSP（core 0）的 timer handler 调用。
///
/// SMP 系统中每个核心独立收到 timer 中断，但全局 tick 只由 BSP 递增，
/// 避免多核并发递增导致 tick 值膨胀（N 核 = N 倍速）。
/// 非 BSP 核心调用此函数时直接返回当前值，不递增。
#[inline]
pub fn tick_advance() -> u64 {
    if per_cpu::current_core_id() == 0 {
        TICK_COUNT.fetch_add(1, Ordering::Release) + 1
    } else {
        TICK_COUNT.load(Ordering::Acquire)
    }
}

/// 读取当前 tick 计数。
#[inline]
pub fn get_current_tick() -> u64 {
    TICK_COUNT.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 tick_advance 正确递增计数器（宿主机主线程 core_id 通常为 0）。
    #[test]
    fn tick_advance_increments() {
        let before = get_current_tick();
        let returned = tick_advance();
        assert!(returned >= before);
    }

    /// 验证 get_current_tick 返回非递减值。
    #[test]
    fn tick_monotonic() {
        let a = get_current_tick();
        let b = get_current_tick();
        assert!(b >= a);
    }
}
