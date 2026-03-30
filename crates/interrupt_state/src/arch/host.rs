//! 宿主机中断模拟——用 `AtomicBool` 模拟中断使能位，使 `cargo test` 能真正验证
//! 保存-恢复逻辑，而非空操作走过场。

use core::sync::atomic::{AtomicBool, Ordering};

use super::InterruptArch;

/// 模拟的中断使能位，初始为关闭（与裸机 boot 一致）。
static IRQ_ENABLED: AtomicBool = AtomicBool::new(false);

pub(crate) struct Host;

impl InterruptArch for Host {
    #[inline(always)]
    fn irq_enabled() -> bool {
        IRQ_ENABLED.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn irq_disable() {
        IRQ_ENABLED.store(false, Ordering::Relaxed);
    }

    #[inline(always)]
    unsafe fn irq_enable() {
        IRQ_ENABLED.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
pub(crate) fn set_irq_enabled(enabled: bool) {
    IRQ_ENABLED.store(enabled, Ordering::Relaxed);
}
