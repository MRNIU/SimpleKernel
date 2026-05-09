//! RISC-V S-mode MMU 启用流程。

const SATP_MODE_SV39: usize = 8;
const SATP_MODE_SHIFT: usize = 60;

fn satp_value(pt: &paging::PageTable) -> usize {
    let ppn = pt.root_paddr().as_usize() >> config::PAGE_SIZE_BITS;
    (SATP_MODE_SV39 << SATP_MODE_SHIFT) | ppn
}

/// 激活给定页表并启用 Sv39 地址翻译。
///
/// # Safety
/// `pt` 必须包含当前执行路径、内核段、栈和早期设备访问所需的有效映射。
pub(super) unsafe fn activate_page_table(pt: &paging::PageTable) {
    let satp = satp_value(pt);

    // SAFETY: 调用方保证页表映射正确；写入 satp 后立即刷新本核地址转换缓存。
    unsafe {
        core::arch::asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp,
        );
    }
}
