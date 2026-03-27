/// AArch64 通用定时器子系统
///
/// 使用 AArch64 虚拟定时器（CNTV_*_EL0）实现周期性时钟中断。
/// 目标 tick 频率：`config::TIMER_FREQ_HZ` Hz。
use core::sync::atomic::Ordering;

use config::TIMER_FREQ_HZ;

/// 读取当前 tick 计数——委托给 arch-traits 全局计数器
pub fn get_current_tick() -> u64 {
    arch_traits::get_current_tick()
}

/// 返回每秒 tick 数
pub const fn ticks_per_second() -> u64 {
    TIMER_FREQ_HZ
}

/// 读取 CNTFRQ_EL0（定时器硬件频率，Hz）
#[inline]
fn read_cntfrq() -> u64 {
    let freq: u64;
    // SAFETY: CNTFRQ_EL0 在 EL1 下始终可读
    unsafe { core::arch::asm!("mrs {freq}, cntfrq_el0", freq = out(reg) freq) };
    freq
}

/// 计算每个 tick 的定时器计数值
#[inline]
fn get_interval() -> u64 {
    read_cntfrq() / TIMER_FREQ_HZ
}

/// 初始化主核虚拟定时器
///
/// 设置 CNTV_TVAL_EL0 为计算的间隔值，然后使能定时器（CNTV_CTL_EL0 = 1）。
pub fn init() {
    let hw_freq = read_cntfrq();
    let interval = get_interval();
    // SAFETY: CNTV_TVAL_EL0 / CNTV_CTL_EL0 在 EL1 下可读写
    unsafe {
        core::arch::asm!(
            "msr cntv_tval_el0, {interval}",
            "msr cntv_ctl_el0, {one}",
            interval = in(reg) interval,
            one = in(reg) 1u64,
        );
    }
    log::info!(
        "TimerInit: hw_freq={} Hz, tick_freq={} Hz, interval={} cycles",
        hw_freq,
        TIMER_FREQ_HZ,
        interval
    );
}

/// 初始化从核虚拟定时器
pub fn init_smp(cpu_id: usize) {
    let interval = get_interval();
    // SAFETY: CNTV_TVAL_EL0 / CNTV_CTL_EL0 在 EL1 下可读写
    unsafe {
        core::arch::asm!(
            "msr cntv_tval_el0, {interval}",
            "msr cntv_ctl_el0, {one}",
            interval = in(reg) interval,
            one = in(reg) 1u64,
        );
    }
    log::info!("TimerInitSMP core {}", cpu_id);
}

/// 处理定时器中断（虚拟定时器 PPI IRQ 27）
pub fn handle_timer(_ctx: &mut super::context::TrapContext) {
    // 递增全局 tick 计数（arch-traits 管理）
    let tick = arch_traits::tick_advance();

    // 重新加载定时器计数值
    let interval = get_interval();
    // SAFETY: CNTV_TVAL_EL0 在 EL1 下可写
    unsafe {
        core::arch::asm!(
            "msr cntv_tval_el0, {interval}",
            interval = in(reg) interval,
        );
    }

    // 更新 per-CPU 抢占状态
    // SAFETY: 在中断处理程序中调用，中断已被 DAIF 屏蔽
    let per_cpu = unsafe { per_cpu::current_per_cpu() };
    per_cpu.preempt.enter_hardirq();

    // 通过 arch-traits 回调调用 task::timer_tick()（打破 arch→task 依赖）
    arch_traits::call_timer_tick();

    per_cpu.preempt.exit_hardirq();

    // 通知 idle loop 检查调度（唤醒到期睡眠任务等）
    per_cpu.preempt.need_resched.store(true, Ordering::Release);

    log::info!("Tick #{} (core {})", tick, per_cpu::current_core_id());
}
