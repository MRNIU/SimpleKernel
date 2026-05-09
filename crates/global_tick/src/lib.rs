// Copyright The SimpleKernel Contributors

//! 全局 tick 计数器——内核调度时基。
//!
//! 提供单调递增的 tick 计数，由 BSP 的 timer handler 驱动。
//! 独立于具体的定时器硬件，
//! 使 task/scheduler 等模块无需依赖架构层即可读取时间。
//!
//! per-CPU tick 记账见 [`local_tick`] crate。

#![no_std]

use core::sync::atomic::{AtomicU64, Ordering};

/// 全局 tick 计数器——由 BSP 的 timer handler 递增。
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// 推进 tick 计数器——由各核心的 timer handler 调用。
///
/// `is_bsp == true` 时递增并返回新值；
/// 其余核心直接返回当前值，避免 N 核并发递增导致 tick 膨胀。
#[inline]
pub fn advance(is_bsp: bool) -> u64 {
    if is_bsp {
        TICK_COUNT.fetch_add(1, Ordering::AcqRel) + 1
    } else {
        TICK_COUNT.load(Ordering::Acquire)
    }
}

/// 读取当前 tick 计数。
#[inline]
pub fn current() -> u64 {
    TICK_COUNT.load(Ordering::Acquire)
}
