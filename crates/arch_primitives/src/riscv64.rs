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

    /// # Safety
    ///
    /// 调用方必须满足 [`ArchImpl::set_percpu_base`] 的契约，保证 `val` 指向
    /// 当前 hart 的有效 per-CPU 区域。
    #[inline(always)]
    unsafe fn set_percpu_base(val: usize) {
        // SAFETY: 上层安全契约保证 `val` 是当前 hart 的 per-CPU 基地址。
        // 若写入错误地址，后续通过 TP 定位的 per-CPU 数据会落到错误内存。
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

    /// # Safety
    ///
    /// 调用方必须满足 [`ArchImpl::enable_irq`] 的契约，确保 trap 向量、栈和临界区状态
    /// 已允许处理中断。
    #[inline(always)]
    unsafe fn enable_irq() {
        // SAFETY: 上层安全契约保证中断向量表和当前上下文可处理中断。
        // 若在关中断锁或未初始化 trap 的状态下启用，可能导致重入死锁或异常跳转。
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
