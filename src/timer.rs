// Copyright The SimpleKernel Contributors

use core::sync::atomic::{AtomicUsize, Ordering};

use config::TIMER_FREQ_HZ;

static TIMEKEEPER_CORE_ID: AtomicUsize = AtomicUsize::new(usize::MAX);

/// 设置负责推进全局 tick 的 CPU。
///
/// # Panics
/// 当 timekeeper 已经初始化过时 panic。
pub fn init_timekeeper(core_id: usize) {
    TIMEKEEPER_CORE_ID
        .compare_exchange(usize::MAX, core_id, Ordering::AcqRel, Ordering::Acquire)
        .expect("TimerInit: timekeeper 已初始化");
}

/// 返回负责推进全局 tick 的 CPU。
///
/// # Panics
/// 当 timekeeper 尚未初始化时 panic。
pub fn timekeeper_core_id() -> usize {
    let core_id = TIMEKEEPER_CORE_ID.load(Ordering::Acquire);
    assert_ne!(core_id, usize::MAX, "TimerInit: timekeeper 未初始化");
    core_id
}

/// 校验并计算每个 tick 的硬件计数间隔。
///
/// `freq` 必须不小于 [`TIMER_FREQ_HZ`]，否则整除结果会变成 0，timer
/// 可能立即重触发或静默失效。启动期遇到这种平台配置错误应 fail-fast。
///
/// # Panics
/// 当 `freq < TIMER_FREQ_HZ` 或计算出的 tick interval 为 0 时 panic。
pub fn checked_tick_interval(freq: u64) -> u64 {
    assert!(
        freq >= TIMER_FREQ_HZ,
        "TimerInit: hw_freq={} Hz 小于 tick_freq={} Hz，interval 会变成 0",
        freq,
        TIMER_FREQ_HZ
    );

    let interval = freq / TIMER_FREQ_HZ;
    assert!(
        interval > 0,
        "TimerInit: hw_freq={} Hz, tick_freq={} Hz 计算出零 interval",
        freq,
        TIMER_FREQ_HZ
    );
    interval
}

/// 架构无关的定时器 tick 处理——由各架构 timer handler 在重置定时器后调用。
///
/// 调用方（架构中断入口）负责 `HardIrqGuard` 的生命周期管理。
///
/// 职责：
/// 1. 递增全局 tick 计数（timekeeper）和 per-CPU tick 计数（每核）
/// 2. 调用 task::timer_tick() 推进调度记账
/// 3. 若调度策略要求抢占，则标记当前核在 IRQ exit 后调度
/// 4. 日志
///
/// # Panics
/// 当 timekeeper 或任务调度基础设施尚未初始化时，底层访问会 fail-fast。
pub fn handle_timer_common() {
    let core_id = per_cpu::current_core_id();
    let tick = global_tick::advance(core_id == timekeeper_core_id());
    let local = local_tick::advance();

    if crate::task::timer_tick() {
        crate::preempt::request_current_core_reschedule();
    }

    log::info!("Tick #{} (core {} local #{})", tick, core_id, local);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// timer 频率低于目标 tick 频率时必须 fail-fast，避免 interval 变成 0。
    #[test]
    #[should_panic(expected = "interval 会变成 0")]
    fn checked_tick_interval_rejects_too_low_frequency() {
        let _ = checked_tick_interval(config::TIMER_FREQ_HZ - 1);
    }

    /// 合法频率应返回非零 tick 间隔。
    #[test]
    fn checked_tick_interval_returns_nonzero_interval() {
        let interval = checked_tick_interval(config::TIMER_FREQ_HZ * 10);
        assert_eq!(interval, 10);
    }
}
