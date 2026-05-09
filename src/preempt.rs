// Copyright The SimpleKernel Contributors

//! 抢占控制——re-export `interrupt_state` 的抢占原语。
//!
//! 抢占状态（`PREEMPT_DISABLE_COUNT`、`NEED_RESCHED`）及其操作函数
//! 定义在 `interrupt_state` crate 中，供 `sync` crate 直接访问。
//! 本模块为内核代码提供便捷的 re-export。

pub use interrupt_state::NEED_RESCHED;
pub use interrupt_state::check_and_clear_need_resched;
pub use interrupt_state::preemptible;
pub use interrupt_state::set_need_resched_on;

/// 请求当前核心在退出硬中断后执行一次调度。
///
/// 这里只设置 per-CPU `need_resched` 标志，不会在调用点直接切换上下文。
pub fn request_current_core_reschedule() {
    NEED_RESCHED
        .get()
        .store(true, core::sync::atomic::Ordering::Release);
}

/// 在 IRQ exit 路径消费一次抢占请求。
///
/// 若当前仍处于不可抢占区间，则保留 `need_resched`，等待后续出口再次检查。
/// 返回 `true` 表示调用方应在当前出口执行一次调度。
pub fn take_irq_exit_preemption_request() -> bool {
    preemptible() && check_and_clear_need_resched()
}
