/// RISC-V 64 定时器子系统
///
/// 通过 SBI set_timer 接口实现周期性时钟中断。
/// 从 BASIC_INFO.timer_freq（FDT `timebase-frequency`）读取硬件频率，
/// 以 `config::TIMER_FREQ_HZ` 为目标 tick 频率计算触发间隔。
use core::sync::atomic::{AtomicU64, Ordering};

use crate::config::TIMER_FREQ_HZ;

/// 全局 tick 计数器
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// 硬件定时器频率（Hz）——init 时从 BASIC_INFO 读取并缓存
static HW_FREQ: AtomicU64 = AtomicU64::new(0);

/// 读取当前 tick 计数（Acquire 语序，确保看到最新值）
pub fn get_current_tick() -> u64 {
    TICK_COUNT.load(Ordering::Acquire)
}

/// 返回每秒 tick 数
pub const fn ticks_per_second() -> u64 {
    TIMER_FREQ_HZ
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
    freq / TIMER_FREQ_HZ
}

/// 初始化主核定时器
///
/// 从 BASIC_INFO 读取硬件频率，计算 tick 间隔，设置首个超时。
pub fn init() {
    let info = crate::boot_info::BASIC_INFO
        .get()
        .expect("TimerInit: BASIC_INFO 未初始化");
    let freq = info.timer_freq;
    assert!(
        freq > 0,
        "TimerInit: timebase-frequency 为 0，FDT 缺少该属性"
    );
    HW_FREQ.store(freq, Ordering::Relaxed);

    let interval = freq / TIMER_FREQ_HZ;
    let next = read_time() + interval;
    sbi_rt::set_timer(next).ok();
    log::info!(
        "TimerInit: hw_freq={} Hz, tick_freq={} Hz, interval={} cycles",
        freq,
        TIMER_FREQ_HZ,
        interval
    );
}

/// 初始化从核定时器
///
/// # 参数
/// - `hart_id`：当前从核的 hart ID
pub fn init_smp(hart_id: usize) {
    let next = read_time() + get_interval();
    sbi_rt::set_timer(next).ok();
    log::info!("TimerInitSMP core {}", hart_id);
}

/// 处理定时器中断
///
/// 递增 tick 计数，重新设置下一次超时。
///
/// 由 `interrupt.rs` 的 `HandleTrap` 在检测到定时器中断（scause=0x8000_0000_0000_0005）时调用。
pub fn handle_timer() {
    let interval = get_interval();
    // 防御性检查：HW_FREQ 未初始化时 interval == 0，
    // 此时不递增 tick，直接设置一个安全间隔避免中断风暴
    if interval == 0 {
        sbi_rt::set_timer(read_time() + 10_000_000).ok();
        return;
    }

    // 递增 tick 计数
    // 使用 Release 语序：确保 tick 更新对其他核心（通过 Acquire 读取）可见
    let tick = TICK_COUNT.fetch_add(1, Ordering::Release) + 1;

    // 重新设置下一次超时
    let next = read_time() + interval;
    sbi_rt::set_timer(next).ok();

    // 更新 per-CPU 抢占状态
    // SAFETY: 在中断处理程序中调用，此时中断已被 CPU 自动关闭（sstatus.SIE=0）
    let per_cpu = unsafe { crate::per_cpu::current_per_cpu() };
    per_cpu.preempt.enter_hardirq();

    per_cpu.preempt.exit_hardirq();

    // 通知 idle loop 检查调度
    per_cpu.preempt.need_resched.store(true, Ordering::Release);

    if tick % (TIMER_FREQ_HZ / 10) == 0 {
        let core_id = crate::per_cpu::current_core_id();
        log::info!("Tick #{} (core {})", tick, core_id);
    }
}
