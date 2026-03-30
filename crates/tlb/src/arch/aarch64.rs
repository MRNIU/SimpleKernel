//! AArch64 TLB 刷新——`tlbi` / `dsb` / `isb` 指令。
//!
//! [Arm ARM §D8.3](https://developer.arm.com/documentation/ddi0487/latest)
//
// TODO: `aarch64-cpu` crate 未提供 TLBI 指令封装（仅有 DSB/ISB barrier），
// 因此此处使用内联汇编。后续应向上游提交 PR 添加 `tlbi_vmalle1()` 和
// `tlbi_vae1(vaddr)` 封装，替换手写汇编。
// 上游仓库：https://github.com/rust-embedded/aarch64-cpu

use super::TlbArch;

pub(crate) struct Aarch64;

impl TlbArch for Aarch64 {
    #[inline(always)]
    fn flush_all() {
        // SAFETY: tlbi/dsb/isb 是 EL1 特权指令。
        unsafe {
            core::arch::asm!("tlbi vmalle1", "dsb sy", "isb");
        }
    }

    #[inline(always)]
    fn flush_page(vaddr: usize) {
        // SAFETY: tlbi/dsb/isb 是 EL1 特权指令。
        // TLBI VAE1 操作数格式：虚拟地址右移 PAGE_SHIFT 位（页号）。
        let page = vaddr >> config::PAGE_SIZE.trailing_zeros();
        unsafe {
            core::arch::asm!(
                "tlbi vae1, {page}",
                "dsb sy",
                "isb",
                page = in(reg) page,
            );
        }
    }
}
