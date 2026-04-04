//! Per-CPU tick 计数器——每核独立的调度时基。
//!
//! 每个核心的 timer handler 调用 [`advance()`] 递增本核计数器。
//! 调度器等上层模块通过 [`current()`] 读取本核已过 tick 数，
//! 用于时间片记账、负载统计等。
//!
//! 全局 tick 计数器见 [`global_tick`] crate。
//!
//! # 并发安全
//!
//! 底层使用 `#[cpu_local]` 变量（每 CPU 独立副本），
//! 配合 `AtomicU64` 消除同核中断嵌套的潜在竞态，
//! 全部 API 均为 safe 函数。

#![cfg_attr(not(test), no_std)]

use core::sync::atomic::{AtomicU64, Ordering};

use per_cpu::cpu_local;

/// 本核 tick 计数器。
///
/// 每次 timer 中断递增 1，单调递增，永不重置。
/// 使用 `AtomicU64` 以避免同核中断嵌套时的可变引用冲突。
#[cpu_local]
static LOCAL_TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// 递增本核 tick 计数并返回新值。
///
/// 由本核 timer handler 在中断上下文中调用。
#[inline]
pub fn advance() -> u64 {
    LOCAL_TICK_COUNT.get().fetch_add(1, Ordering::Relaxed) + 1
}

/// 读取本核当前 tick 计数。
#[inline]
pub fn current() -> u64 {
    LOCAL_TICK_COUNT.get().load(Ordering::Relaxed)
}
