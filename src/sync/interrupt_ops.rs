/// 公共中断控制接口
///
/// 通过 `ArchOps` trait 分派到具体架构实现，测试模式下为 no-op 存根。
/// P4+ 多个模块需要直接使用中断控制：
/// - `SpinLock`：加锁/解锁时禁用/恢复中断
/// - 定时器 handler：`hardirq_count` 维护
/// - P5 调度器：`kernel_thread_bootstrap` 中启用中断、`Schedule` 中的中断保存/恢复

#[cfg(not(test))]
use crate::arch::ArchOps;

/// 查询当前中断是否启用
#[inline(always)]
pub fn get_status() -> bool {
    #[cfg(not(test))]
    {
        crate::arch::Arch::irq_enabled()
    }
    #[cfg(test)]
    {
        false
    }
}

/// 禁用中断
#[inline(always)]
pub fn disable() {
    #[cfg(not(test))]
    crate::arch::Arch::irq_disable();
}

/// 启用中断
///
/// # Safety
/// 调用方必须确保在启用中断后不会破坏当前临界区的不变量。
#[inline(always)]
pub unsafe fn enable() {
    #[cfg(not(test))]
    // SAFETY: 由调用方保证安全性
    unsafe {
        crate::arch::Arch::irq_enable()
    };
}

/// RAII 中断保存/恢复守卫
///
/// 构造时保存中断状态并禁用中断，析构时恢复之前的状态。
pub struct InterruptGuard {
    saved: bool,
}

impl InterruptGuard {
    /// 保存当前中断状态并禁用中断
    pub fn new() -> Self {
        let saved = get_status();
        disable();
        Self { saved }
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if self.saved {
            // SAFETY: 恢复到进入守卫前的中断状态
            unsafe { enable() };
        }
    }
}
