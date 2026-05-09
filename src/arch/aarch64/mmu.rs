// Copyright The SimpleKernel Contributors

//! AArch64 EL1 MMU 启用流程。

const MAIR_NORMAL_WB: u64 = 0xFF;

const TCR_T0SZ_48_BITS: u64 = 16;
const TCR_IRGN0_WRITE_BACK: u64 = 0b01 << 8;
const TCR_ORGN0_WRITE_BACK: u64 = 0b01 << 10;
const TCR_SH0_INNER_SHAREABLE: u64 = 0b11 << 12;
const TCR_IPS_48_BITS: u64 = 0b101;
const TCR_IPS_SHIFT: u64 = 32;
const SUPPORTED_PA_BITS: usize = 48;

fn tcr_el1_value() -> u64 {
    TCR_T0SZ_48_BITS
        | TCR_IRGN0_WRITE_BACK
        | TCR_ORGN0_WRITE_BACK
        | TCR_SH0_INNER_SHAREABLE
        | (TCR_IPS_48_BITS << TCR_IPS_SHIFT)
}

fn supported_pa_bits() -> usize {
    let value: u64;
    // SAFETY: ID_AA64MMFR0_EL1 是 EL1 可读的 feature register。
    unsafe { core::arch::asm!("mrs {value}, id_aa64mmfr0_el1", value = out(reg) value) };
    match value & 0b1111 {
        0b0000 => 32,
        0b0001 => 36,
        0b0010 => 40,
        0b0011 => 42,
        0b0100 => 44,
        0b0101 => 48,
        0b0110 => 52,
        0b0111 => 56,
        parange => panic!("AArch64 MMU: 未知 PARange 编码 {:#x}", parange),
    }
}

fn assert_supported_pa_bits() {
    let bits = supported_pa_bits();
    assert!(
        bits == SUPPORTED_PA_BITS,
        "AArch64 MMU: 当前仅支持 {}-bit PA range，硬件报告 {}-bit",
        SUPPORTED_PA_BITS,
        bits
    );
}

/// 激活给定页表并启用 MMU。
///
/// # Safety
/// `pt` 必须包含当前执行路径、内核段、栈和早期 MMIO 所需的有效映射。
pub(super) unsafe fn activate_page_table(pt: &paging::PageTable) {
    assert_supported_pa_bits();
    let ttbr = pt.root_paddr().as_usize() as u64;
    let tcr = tcr_el1_value();

    // SAFETY: 调用方保证页表映射正确；MAIR/TCR 必须在写入 TTBR 并使能 MMU 前配置。
    unsafe {
        core::arch::asm!(
            "msr mair_el1, {mair}",
            "msr tcr_el1, {tcr}",
            "isb",
            "msr ttbr0_el1, {ttbr}",
            "isb",
            "tlbi vmalle1",
            "dsb sy",
            "isb",
            mair = in(reg) MAIR_NORMAL_WB,
            tcr = in(reg) tcr,
            ttbr = in(reg) ttbr,
        );

        let mut sctlr: u64;
        core::arch::asm!("mrs {sctlr}, sctlr_el1", sctlr = out(reg) sctlr);
        sctlr |= 1;
        core::arch::asm!(
            "msr sctlr_el1, {sctlr}",
            "isb",
            sctlr = in(reg) sctlr,
        );
    }
}
