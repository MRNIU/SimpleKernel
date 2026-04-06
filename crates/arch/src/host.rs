//! 宿主机 mock 实现——用于 `cargo test`。
//!
//! - Per-CPU: 直接返回 0（宿主机无 per-CPU 区域）
//! - 中断: `AtomicBool` 模拟中断使能位
//! - TLB: no-op

use core::sync::atomic::{AtomicBool, Ordering};

use super::ArchImpl;

/// 模拟的中断使能位，初始为关闭（与裸机 boot 一致）。
static IRQ_ENABLED: AtomicBool = AtomicBool::new(false);

pub(crate) struct Host;

impl ArchImpl for Host {
    const PA_BITS: usize = 48;
    const PT_LEVELS: usize = 3;

    #[inline(always)]
    fn percpu_base() -> usize {
        0
    }

    #[inline(always)]
    unsafe fn set_percpu_base(_val: usize) {
        // 宿主机上 per-CPU 基地址无意义
    }

    #[inline(always)]
    fn core_id() -> usize {
        0
    }

    #[inline(always)]
    fn is_irq_enabled() -> bool {
        IRQ_ENABLED.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn disable_irq() {
        IRQ_ENABLED.store(false, Ordering::Relaxed);
    }

    #[inline(always)]
    unsafe fn enable_irq() {
        IRQ_ENABLED.store(true, Ordering::Relaxed);
    }

    #[inline(always)]
    fn flush_tlb_all() {}

    #[inline(always)]
    fn flush_tlb_page(_vaddr: usize) {}
}

/// 设置模拟的中断使能状态（测试辅助函数）。
pub(crate) fn set_irq_enabled(enabled: bool) {
    IRQ_ENABLED.store(enabled, Ordering::Relaxed);
}
