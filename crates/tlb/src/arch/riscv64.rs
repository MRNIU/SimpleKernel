//! RISC-V TLB 刷新——`sfence.vma` 指令。
//!
//! [RISC-V Privileged Spec §4.2.1](https://github.com/riscv/riscv-isa-manual)

use super::TlbArch;

pub(crate) struct Riscv64;

impl TlbArch for Riscv64 {
    #[inline(always)]
    fn flush_all() {
        riscv::asm::sfence_vma_all();
    }

    #[inline(always)]
    fn flush_page(vaddr: usize) {
        // asid = 0：刷新所有 ASID 下该地址的 TLB 条目
        riscv::asm::sfence_vma(0, vaddr);
    }
}
