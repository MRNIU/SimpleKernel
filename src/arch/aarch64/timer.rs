// Copyright The SimpleKernel Contributors

/// AArch64 通用定时器子系统
///
/// 使用 AArch64 虚拟定时器（CNTV_*_EL0）实现周期性时钟中断。
/// 目标 tick 频率：`config::TIMER_FREQ_HZ` Hz。
use core::sync::atomic::{AtomicU64, Ordering};

use config::TIMER_FREQ_HZ;
use per_cpu::cpu_local;

/// 本核下一次 absolute timer deadline。
#[cpu_local]
static NEXT_DEADLINE: AtomicU64 = AtomicU64::new(0);

/// 读取 CNTFRQ_EL0（定时器硬件频率，Hz）
#[inline]
fn read_cntfrq() -> u64 {
    let freq: u64;
    // SAFETY: CNTFRQ_EL0 在 EL1 下始终可读
    unsafe { core::arch::asm!("mrs {freq}, cntfrq_el0", freq = out(reg) freq) };
    freq
}

/// 读取 CNTVCT_EL0（虚拟计数器当前值）。
#[inline]
fn read_cntvct() -> u64 {
    let counter: u64;
    // SAFETY: CNTVCT_EL0 在 EL1 下始终可读
    unsafe { core::arch::asm!("mrs {counter}, cntvct_el0", counter = out(reg) counter) };
    counter
}

/// 计算每个 tick 的定时器计数值
#[inline]
fn get_interval() -> u64 {
    crate::timer::checked_tick_interval(read_cntfrq())
}

/// 写入 CNTV_CVAL_EL0，并确保虚拟 timer 处于启用状态。
fn program_deadline(deadline: u64) {
    // SAFETY: CNTV_CVAL_EL0 / CNTV_CTL_EL0 在 EL1 下可写
    unsafe {
        core::arch::asm!(
            "msr cntv_cval_el0, {deadline}",
            "msr cntv_ctl_el0, {one}",
            deadline = in(reg) deadline,
            one = in(reg) 1u64,
        );
    }
}

/// 根据当前虚拟计数器初始化本核 absolute deadline。
fn init_next_deadline(interval: u64, context: &str) {
    let deadline = read_cntvct()
        .checked_add(interval)
        .unwrap_or_else(|| panic!("TimerInit: {context} deadline 溢出: interval={interval}"));
    NEXT_DEADLINE.get().store(deadline, Ordering::Relaxed);
    program_deadline(deadline);
}

/// 推进本核 absolute deadline 到未来。
fn reload_next_deadline(interval: u64) {
    let current = NEXT_DEADLINE.get().load(Ordering::Relaxed);
    assert_ne!(
        current, 0,
        "TimerInit: AArch64 next_deadline 未初始化，不能重装 timer"
    );
    let next = crate::timer::next_absolute_deadline(current, read_cntvct(), interval);
    NEXT_DEADLINE.get().store(next, Ordering::Relaxed);
    program_deadline(next);
}

/// 初始化主核虚拟定时器
///
/// 设置 CNTV_CVAL_EL0 为 absolute deadline，然后使能定时器（CNTV_CTL_EL0 = 1）。
pub fn init() {
    let hw_freq = read_cntfrq();
    let interval = get_interval();
    init_next_deadline(interval, "primary init");
    log::info!(
        "TimerInit: hw_freq={} Hz, tick_freq={} Hz, interval={} cycles",
        hw_freq,
        TIMER_FREQ_HZ,
        interval
    );
}

/// 初始化从核虚拟定时器
pub fn init_smp(cpu_id: usize) {
    init_next_deadline(get_interval(), "smp init");
    log::info!("TimerInitSMP core {}", cpu_id);
}

/// 处理定时器中断（虚拟定时器 PPI IRQ 27）
///
/// 重置定时器硬件后，调用架构无关的公共 tick 处理。
pub fn handle_timer(_ctx: &mut super::context::TrapContext) {
    let interval = get_interval();
    reload_next_deadline(interval);

    // 架构无关：tick 计数、抢占状态、调度记账、日志
    crate::timer::handle_timer_common();
}
