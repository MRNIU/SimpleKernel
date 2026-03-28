//! TLB 管理——架构无关的 TLB 刷新接口。
//!
//! 使用 `target_os = "none"` 区分裸机和宿主机编译。

/// 刷新整个 TLB——用于批量页表操作（切换地址空间、初始化映射等）。
///
/// 单页 unmap 应使用 [`flush_tlb_page`] 避免不必要的全局刷新。
#[inline(always)]
pub fn flush_tlb() {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    riscv::asm::sfence_vma_all();
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    // SAFETY: tlbi/dsb/isb 是 EL1 特权指令。
    // aarch64-cpu crate 不提供 TLBI 封装，只能使用内联汇编。
    unsafe {
        core::arch::asm!("tlbi vmalle1", "dsb sy", "isb");
    }
    // 宿主机: no-op
}

/// 刷新指定虚拟地址对应的单条 TLB 表项。
///
/// 在 unmap 单页或修改单个 PTE 后调用，比 [`flush_tlb`] 精确、开销更低。
/// 参考 Theseus 的 `tlb::flush_virt_addr` 和 Linux 的 `flush_tlb_page`。
#[inline(always)]
pub fn flush_tlb_page(vaddr: usize) {
    #[cfg(all(target_os = "none", target_arch = "riscv64"))]
    // SAFETY: sfence.vma 是 S-mode 特权指令。
    // rs1 = vaddr（虚拟地址），rs2 = x0（所有 ASID）。
    unsafe {
        core::arch::asm!(
            "sfence.vma {vaddr}, zero",
            vaddr = in(reg) vaddr,
        );
    }
    #[cfg(all(target_os = "none", target_arch = "aarch64"))]
    // SAFETY: tlbi/dsb/isb 是 EL1 特权指令。
    // TLBI VAE1 操作数格式：虚拟地址右移 12 位（页号），低 48 位有效。
    unsafe {
        core::arch::asm!(
            "tlbi vae1, {page}",
            "dsb sy",
            "isb",
            page = in(reg) vaddr >> 12,
        );
    }
    let _ = vaddr; // 宿主机: no-op，消除 unused 警告
}
