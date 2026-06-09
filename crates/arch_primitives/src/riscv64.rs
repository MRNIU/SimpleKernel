// Copyright The SimpleKernel Contributors

//! RISC-V 64 架构实现——S 模式。
//!
//! - Per-CPU: TP 寄存器
//! - 中断: `sstatus.SIE` 位
//! - TLB: `sfence.vma` 指令
//!
//! 参考：
//! - [RISC-V Privileged Spec §4.1.1](https://github.com/riscv/riscv-isa-manual)（sstatus.SIE）
//! - [RISC-V Privileged Spec §4.2.1](https://github.com/riscv/riscv-isa-manual)（sfence.vma）

use super::ArchImpl;

pub(crate) struct Riscv64;

impl ArchImpl for Riscv64 {
    const PA_BITS: usize = 56;
    const PT_LEVELS: usize = 3;
    const FDT_INTERRUPT_CONTROLLER_COMPATIBLES: &'static [&'static str] =
        &["riscv,plic0", "sifive,plic-1.0.0"];

    #[inline(always)]
    fn percpu_base() -> usize {
        let val: usize;
        // SAFETY: TP 在 S 模式下始终可读
        unsafe { core::arch::asm!("mv {}, tp", out(reg) val) };
        val
    }

    #[inline(always)]
    unsafe fn set_percpu_base(val: usize) {
        // SAFETY: 由调用方保证设置正确的 per-CPU 基地址
        unsafe { core::arch::asm!("mv tp, {}", in(reg) val) };
    }

    #[inline(always)]
    fn core_id() -> usize {
        // boot.S 已将 hart_id 写入 TP，初始化前直接读取
        Self::percpu_base()
    }

    #[inline(always)]
    fn is_irq_enabled() -> bool {
        riscv::register::sstatus::read().sie()
    }

    #[inline(always)]
    fn disable_irq() {
        riscv::interrupt::supervisor::disable();
    }

    #[inline(always)]
    unsafe fn enable_irq() {
        // SAFETY: 由调用方保证中断向量表就绪、不在关中断临界区内
        unsafe { riscv::interrupt::supervisor::enable() };
    }

    #[inline(always)]
    fn flush_tlb_all() {
        riscv::asm::sfence_vma_all();
    }

    #[inline(always)]
    fn flush_tlb_page(vaddr: usize) {
        // asid = 0：刷新所有 ASID 下该地址的 TLB 条目
        riscv::asm::sfence_vma(0, vaddr);
    }
}
