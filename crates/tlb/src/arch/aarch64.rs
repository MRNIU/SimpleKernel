//! AArch64 TLB 刷新——使用 `aarch64-cpu` crate 的 TLBI / barrier 封装。
//!
//! [Arm ARM §D8.3](https://developer.arm.com/documentation/ddi0487/latest)

use aarch64_cpu::asm::{barrier, tlbi};

use super::TlbArch;

pub(crate) struct Aarch64;

impl TlbArch for Aarch64 {
    #[inline(always)]
    fn flush_all() {
        // SAFETY: TLBI VMALLE1 + DSB + ISB 是 EL1 特权指令，
        // 调用方在内核态（EL1）执行。
        barrier::dsb(barrier::SY);
        tlbi::vmalle1();
        barrier::dsb(barrier::SY);
        barrier::isb(barrier::SY);
    }

    #[inline(always)]
    fn flush_page(vaddr: usize) {
        // SAFETY: TLBI VAE1 + DSB + ISB 是 EL1 特权指令。
        // Addr::new 负责将虚拟地址右移 12 位并编码到正确的位域。
        // ASID 传 0——当前为单地址空间，不区分 ASID。
        barrier::dsb(barrier::SY);
        tlbi::vae1(tlbi::Addr::new(vaddr as u64, 0));
        barrier::dsb(barrier::SY);
        barrier::isb(barrier::SY);
    }
}
