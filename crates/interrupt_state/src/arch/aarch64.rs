//! AArch64 中断控制实现——通过 `DAIF` 寄存器的 I 位控制 IRQ 屏蔽。
//!
//! 参考 [Arm ARM §D1.7](https://developer.arm.com/documentation/ddi0487/)
//! DAIF 中断屏蔽位。

use aarch64_cpu::registers::{DAIF, Readable};

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
        // SAFETY: daifset 是 EL1 特权指令，原子地设置 DAIF 位。
        // 不能使用 DAIF.write()（msr DAIF, Xn），因为它会覆盖整个寄存器，
        // 可能意外取消屏蔽 Debug/SError/FIQ 异常。
        // TODO: aarch64-cpu crate 未封装 daifset/daifclr，待上游提供后替换裸汇编
        unsafe {
            core::arch::asm!("msr daifset, #2");
        }
    }

    #[inline(always)]
    unsafe fn irq_enable() {
        // SAFETY: daifclr 是 EL1 特权指令，原子地清除 DAIF 位。
        // TODO: 同 irq_disable，待 aarch64-cpu 封装 daifclr 后替换
        unsafe {
            core::arch::asm!("msr daifclr, #2");
        }
    }
}
