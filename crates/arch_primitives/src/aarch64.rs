// Copyright The SimpleKernel Contributors

//! AArch64 架构实现——EL1。
//!
//! - Per-CPU: `TPIDR_EL1` 寄存器
//! - 中断: `DAIF.I` 位
//! - TLB: `TLBI` + `DSB` + `ISB` 指令序列
//!
//! 参考：
//! - [Arm ARM §D1.7](https://developer.arm.com/documentation/ddi0487/)（DAIF 中断屏蔽位）
//! - [Arm ARM §D8.3](https://developer.arm.com/documentation/ddi0487/)（TLB 维护指令）

use aarch64_cpu::{
    asm::{barrier, tlbi},
    registers::{DAIF, DAIFClr, DAIFSet, MPIDR_EL1, Readable, Writeable},
};

use super::ArchImpl;

pub(crate) struct Aarch64;

impl ArchImpl for Aarch64 {
    const PA_BITS: usize = 44;
    const PT_LEVELS: usize = 4;
    const FDT_INTERRUPT_CONTROLLER_COMPATIBLES: &'static [&'static str] = &["arm,gic-v3"];

    #[inline(always)]
    fn percpu_base() -> usize {
        let val: usize;
        // SAFETY: TPIDR_EL1 在 EL1 下始终可读
        unsafe { core::arch::asm!("mrs {}, tpidr_el1", out(reg) val) };
        val
    }

    #[inline(always)]
    unsafe fn set_percpu_base(val: usize) {
        // SAFETY: 由调用方保证设置正确的 per-CPU 基地址
        unsafe { core::arch::asm!("msr tpidr_el1, {}", in(reg) val) };
    }

    #[inline(always)]
    fn core_id() -> usize {
        MPIDR_EL1.read(MPIDR_EL1::Aff0) as usize
    }

    #[inline(always)]
    fn is_irq_enabled() -> bool {
        // DAIF.I = 0 表示 IRQ 未屏蔽（即中断启用）
        DAIF.read(DAIF::I) == 0
    }

    #[inline(always)]
    fn disable_irq() {
        DAIFSet.write(DAIFSet::I::Mask);
    }

    #[inline(always)]
    unsafe fn enable_irq() {
        DAIFClr.write(DAIFClr::I::Unmask);
    }

    #[inline(always)]
    fn flush_tlb_all() {
        // SAFETY: TLBI VMALLE1 + DSB + ISB 是 EL1 特权指令，
        // 调用方在内核态（EL1）执行。
        barrier::dsb(barrier::SY);
        tlbi::vmalle1();
        barrier::dsb(barrier::SY);
        barrier::isb(barrier::SY);
    }

    #[inline(always)]
    fn flush_tlb_page(vaddr: usize) {
        // SAFETY: TLBI VAE1 + DSB + ISB 是 EL1 特权指令。
        // Addr::new 负责将虚拟地址右移 12 位并编码到正确的位域。
        // ASID 传 0——当前为单地址空间，不区分 ASID。
        barrier::dsb(barrier::SY);
        tlbi::vae1(tlbi::Addr::new(vaddr as u64, 0));
        barrier::dsb(barrier::SY);
        barrier::isb(barrier::SY);
    }
}
