/// 架构无关的定时器 tick 处理——由各架构 timer handler 在重置定时器后调用。
///
/// 职责：
/// 1. 递增全局 tick 计数（BSP）和 per-CPU tick 计数（每核）
/// 2. 更新 per-CPU 抢占状态（HardIrqGuard RAII）
/// 3. 调用 task::timer_tick() 推进调度记账
/// 4. 标记 need_resched
/// 5. 日志
pub fn handle_timer_common() {
    let _irq = interrupt_state::HardIrqGuard::enter();

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
