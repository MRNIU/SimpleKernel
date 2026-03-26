/// RISC-V 64 定时器子系统
///
/// 通过 SBI legacy set_timer 接口实现周期性时钟中断。
/// 每 `TIMER_INTERVAL` 个时钟周期触发一次，约 1 Hz（假设 10 MHz 时钟）。
use core::sync::atomic::{AtomicU64, Ordering};

/// 定时器触发间隔（时钟周期数）
///
/// 假设 RISC-V 参考时钟约为 10 MHz，则 10_000_000 周期 ≈ 1 秒。
pub const TIMER_INTERVAL: u64 = 10_000_000;

/// 全局 tick 计数器
static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// 读取当前 tick 计数（Acquire 语序，确保看到最新值）
pub fn get_current_tick() -> u64 {
    TICK_COUNT.load(Ordering::Acquire)
}

/// 返回每秒 tick 数（RISC-V：1 Hz）
pub const fn ticks_per_second() -> u64 {
    1
}

/// 读取 `time` CSR（参考时钟周期计数）
///
/// # Safety
/// `rdtime` 是非特权指令，S 模式下始终可读。
#[inline]
fn read_time() -> u64 {
    let time: u64;
    // SAFETY: rdtime 是非特权指令，在 S 模式下可安全执行
    unsafe { core::arch::asm!("rdtime {time}", time = out(reg) time) };
    time
}

/// 初始化主核定时器
///
/// 通过 SBI legacy set_timer 设置第一次超时，使能 S 模式定时器中断。
/// 注意：SIE.STIE 位由 `interrupt_init()` 统一使能，此处只负责设置首个超时值。
pub fn init() {
    let next = read_time() + TIMER_INTERVAL;
    sbi_rt::set_timer(next).ok();
    log::info!("TimerInit: 10MHz, 1Hz tick");
}

/// 初始化从核定时器
///
/// # 参数
/// - `hart_id`：当前从核的 hart ID
pub fn init_smp(hart_id: usize) {
    let next = read_time() + TIMER_INTERVAL;
    sbi_rt::set_timer(next).ok();
    log::info!("TimerInitSMP core {}", hart_id);
}

/// 处理定时器中断
///
/// 递增 tick 计数，重新设置下一次超时，每 10 个 tick 输出一次日志。
///
/// 由 `interrupt.rs` 的 `HandleTrap` 在检测到定时器中断（scause=0x8000_0000_0000_0005）时调用。
pub fn handle_timer() {
    // 递增 tick 计数
    // 使用 Release 语序：确保 tick 更新对其他核心（通过 Acquire 读取）可见，
    // 为 P5 跨核调度决策提供正确的时序保证
    let tick = TICK_COUNT.fetch_add(1, Ordering::Release) + 1;

    // 重新设置下一次超时
    let next = read_time() + TIMER_INTERVAL;
    sbi_rt::set_timer(next).ok();

    // 更新 per-CPU 抢占状态
    // SAFETY: 在中断处理程序中调用，此时中断已被 CPU 自动关闭（sstatus.SIE=0）
    let per_cpu = unsafe { crate::per_cpu::current_per_cpu() };
    per_cpu.preempt.enter_hardirq();

    per_cpu.preempt.exit_hardirq();

    // 通知 idle loop 检查调度
    per_cpu.preempt.need_resched.store(true, Ordering::Release);

    if tick % 10 == 0 {
        let core_id = crate::per_cpu::current_core_id();
        log::info!("Tick #{} (core {})", tick, core_id);
    }
}
