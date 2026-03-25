/// AArch64 通用定时器子系统
///
/// 使用 AArch64 虚拟定时器（CNTV_*_EL0）实现周期性时钟中断。
/// 目标 tick 频率：`TIMER_FREQ_HZ` Hz（默认 1000 Hz）。
use core::sync::atomic::{AtomicU64, Ordering};

/// 目标 tick 频率（Hz）
pub const TIMER_FREQ_HZ: u64 = 1000;

/// 全局 tick 计数器
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// 读取 CNTFRQ_EL0（定时器硬件频率，Hz）
#[inline]
fn read_cntfrq() -> u64 {
    let freq: u64;
    // SAFETY: CNTFRQ_EL0 在 EL1 下可读
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
pub fn timer_init() {
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
        "TimerInit: freq={} Hz, interval={} cycles",
        TIMER_FREQ_HZ,
        interval
    );
}

/// 初始化从核虚拟定时器
///
/// # 参数
/// - `cpu_id`：当前从核的 CPU ID
pub fn timer_init_smp(cpu_id: usize) {
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
/// 递增 tick 计数，重新加载 CNTV_TVAL_EL0，每 10 个 tick 输出日志。
///
/// # 参数
/// - `_ctx`：陷阱上下文指针（当前未使用，为将来抢占调度预留）
pub fn handle_timer(_ctx: &mut super::context::TrapContext) {
    let tick = TICK_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

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
    let per_cpu = unsafe { crate::per_cpu::current_per_cpu() };
    per_cpu.preempt.hardirq_count = per_cpu.preempt.hardirq_count.wrapping_add(1);

    if tick % 10 == 0 {
        let core_id = crate::per_cpu::current_core_id();
        log::info!("Tick #{} (core {})", tick, core_id);
    }
}
