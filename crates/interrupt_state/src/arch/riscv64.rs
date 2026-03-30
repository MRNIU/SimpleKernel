//! RISC-V 64 中断控制实现——通过 `sstatus.SIE` 位控制 S 模式中断。
//!
//! 参考 [RISC-V Privileged Spec §4.1.1](https://github.com/riscv/riscv-isa-manual)
//! `sstatus` 寄存器的 SIE 字段。

use super::InterruptArch;

pub(crate) struct Riscv64;

impl InterruptArch for Riscv64 {
    #[inline(always)]
    fn irq_enabled() -> bool {
        riscv::register::sstatus::read().sie()
    }

    #[inline(always)]
    fn irq_disable() {
        riscv::interrupt::supervisor::disable();
    }

    #[inline(always)]
    unsafe fn irq_enable() {
        // SAFETY: 调用方已确保中断向量表就绪、不在关中断临界区内，
        // 且栈/上下文可安全处理中断（见 `InterruptArch::irq_enable` 文档）。
        unsafe { riscv::interrupt::supervisor::enable() };
    }
}
