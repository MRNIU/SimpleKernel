//! AArch64 中断控制实现——通过 `DAIF` 寄存器的 I 位控制 IRQ 屏蔽。
//!
//! 参考 [Arm ARM §D1.7](https://developer.arm.com/documentation/ddi0487/)
//! DAIF 中断屏蔽位。

use aarch64_cpu::registers::{DAIF, DAIFClr, DAIFSet, Readable, Writeable};

use super::InterruptArch;

pub(crate) struct Aarch64;

impl InterruptArch for Aarch64 {
    #[inline(always)]
    fn irq_enabled() -> bool {
        // DAIF.I = 0 表示 IRQ 未屏蔽（即中断启用）
        DAIF.read(DAIF::I) == 0
    }

    #[inline(always)]
    fn irq_disable() {
        DAIFSet.write(DAIFSet::I::Mask);
    }

    #[inline(always)]
    unsafe fn irq_enable() {
        DAIFClr.write(DAIFClr::I::Unmask);
    }
}
