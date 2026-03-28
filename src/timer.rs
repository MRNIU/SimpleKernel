/// 架构无关的定时器 tick 处理——由各架构 timer handler 在重置定时器后调用。
///
/// 职责：
/// 1. 递增全局 tick 计数
/// 2. 更新 per-CPU 抢占状态（enter/exit hardirq）
/// 3. 调用 task::timer_tick() 推进调度记账
/// 4. 标记 need_resched
/// 5. 日志
pub fn handle_timer_common() {
    let tick = arch_traits::tick_advance();

    // SAFETY: 在中断处理程序中调用，中断已被 CPU 自动关闭
    unsafe { per_cpu::enter_hardirq() };

    // 直接调用 task::timer_tick()——同属 kernel crate，无需回调间接调用
    crate::task::timer_tick();

    // SAFETY: 与 enter_hardirq 配对
    unsafe { per_cpu::exit_hardirq() };

    per_cpu::NEED_RESCHED
        .get()
        .store(true, core::sync::atomic::Ordering::Release);

    log::info!("Tick #{} (core {})", tick, per_cpu::current_core_id());
}
