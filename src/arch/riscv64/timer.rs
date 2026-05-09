// Copyright The SimpleKernel Contributors

/// RISC-V 64 定时器子系统
///
/// 通过 SBI set_timer 接口实现周期性时钟中断。
/// 硬件频率由 `early_init()` 通过 `set_hw_freq()` 设置（FDT `timebase-frequency`），
/// 以 `config::TIMER_FREQ_HZ` 为目标 tick 频率计算触发间隔。
use core::sync::atomic::{AtomicU64, Ordering};

/// 硬件定时器频率（Hz）——`early_init` 阶段通过 `set_hw_freq()` 设置
static HW_FREQ: AtomicU64 = AtomicU64::new(0);

/// 设置硬件定时器频率（由 `early_init` 在 FDT 解析后调用）
pub fn set_hw_freq(freq: u64) {
    HW_FREQ.store(freq, Ordering::Relaxed);
}

/// 读取 `time` CSR（参考时钟周期计数）
#[inline]
fn read_time() -> u64 {
    let time: u64;
    // SAFETY: rdtime 是非特权指令，在 S 模式下可安全执行
    unsafe { core::arch::asm!("rdtime {time}", time = out(reg) time) };
    time
}

/// 计算每个 tick 的定时器计数值
#[inline]
fn get_interval() -> u64 {
    let freq = HW_FREQ.load(Ordering::Relaxed);
    crate::timer::checked_tick_interval(freq)
}

/// 设置下一次 SBI timer deadline，失败时直接暴露平台错误。
fn set_timer_or_panic(deadline: u64, context: &str) {
    let ret = sbi_rt::set_timer(deadline);
    assert!(
        ret.ok().is_some(),
        "TimerInit: SBI set_timer 失败 ({context}): deadline={}, error={}, value={}",
        deadline,
        ret.error as isize,
        ret.value
    );
}

/// 初始化主核定时器
///
/// 硬件频率已由 `set_hw_freq()` 设置，计算 tick 间隔，设置首个超时。
pub fn init() {
    let freq = HW_FREQ.load(Ordering::Relaxed);
    assert!(
        freq > 0,
        "TimerInit: HW_FREQ 未设置（early_init 未调用 set_hw_freq？）"
    );

    let interval = crate::timer::checked_tick_interval(freq);
    let next = read_time() + interval;
    set_timer_or_panic(next, "primary init");
    log::info!(
        "TimerInit: hw_freq={} Hz, tick_freq={} Hz, interval={} cycles",
        freq,
        config::TIMER_FREQ_HZ,
        interval
    );
}

/// 初始化从核定时器
///
/// # 参数
/// - `hart_id`：当前从核的 hart ID
pub fn init_smp(hart_id: usize) {
    let next = read_time() + get_interval();
    set_timer_or_panic(next, "smp init");
    log::info!("TimerInitSMP core {}", hart_id);
}

/// 处理定时器中断
///
/// 重置定时器硬件后，调用架构无关的公共 tick 处理。
pub fn handle_timer() {
    let interval = get_interval();
    let next = read_time() + interval;
    set_timer_or_panic(next, "interrupt reload");

    // 架构无关：tick 计数、抢占状态、调度记账、日志
    crate::timer::handle_timer_common();
}
