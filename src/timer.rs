// Copyright The SimpleKernel Contributors

use config::TIMER_FREQ_HZ;

/// 校验并计算每个 tick 的硬件计数间隔。
///
/// `freq` 必须不小于 [`TIMER_FREQ_HZ`]，否则整除结果会变成 0，timer
/// 可能立即重触发或静默失效。启动期遇到这种平台配置错误应 fail-fast。
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
/// 1. 递增全局 tick 计数（BSP）和 per-CPU tick 计数（每核）
/// 2. 调用 task::timer_tick() 推进调度记账
/// 3. 标记 need_resched
/// 4. 日志
pub fn handle_timer_common() {
    // TODO: 引入 BSP ID 后替换硬编码的 `== 0`
    let core_id = per_cpu::current_core_id();
    let tick = global_tick::advance(core_id == 0);
    let local = local_tick::advance();

    crate::task::timer_tick();

    crate::preempt::NEED_RESCHED
        .get()
        .store(true, core::sync::atomic::Ordering::Release);

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
