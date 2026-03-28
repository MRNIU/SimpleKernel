/// AArch64 通用定时器子系统
///
/// 使用 AArch64 虚拟定时器（CNTV_*_EL0）实现周期性时钟中断。
/// 目标 tick 频率：`config::TIMER_FREQ_HZ` Hz。
use config::TIMER_FREQ_HZ;

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
///
/// 重置定时器硬件后，调用架构无关的公共 tick 处理。
pub fn handle_timer(_ctx: &mut super::context::TrapContext) {
    // 架构相关：重新加载虚拟定时器计数值
    let interval = get_interval();
    // SAFETY: CNTV_TVAL_EL0 在 EL1 下可写
    unsafe {
        core::arch::asm!(
            "msr cntv_tval_el0, {interval}",
            interval = in(reg) interval,
        );
    }

    // 架构无关：tick 计数、抢占状态、调度记账、日志
    crate::timer::handle_timer_common();
}
