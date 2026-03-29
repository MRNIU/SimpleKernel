//! 跨架构 PTE 编解码测试。

use crate::{PteFlagsOps, PteOps};
use address::PhysAddr;

/// 地址编解码边界测试——验证零地址和高位地址不会污染标志字段。
fn addr_boundary_roundtrip<T: PteOps>()
where
    T::Flags: PartialEq,
{
    let flags = T::Flags::kernel_rw();

    let pa_zero = PhysAddr::new(0);
    let pte_zero = T::new(pa_zero, flags);
    assert_eq!(pte_zero.paddr(), pa_zero);
    assert_eq!(pte_zero.flags(), flags);

    let pa_high = PhysAddr::new(0x00FF_FFFF_F000);
    let pte_high = T::new(pa_high, flags);
    assert_eq!(pte_high.paddr(), pa_high);
    assert_eq!(pte_high.flags(), flags);
}

/// 内核 preset 不应被标记为用户态可访问。
fn kernel_presets_not_user<F: PteFlagsOps>() {
    assert!(!F::kernel_rw().is_user());
    assert!(!F::kernel_rx().is_user());
    assert!(!F::kernel_ro().is_user());
    assert!(!F::kernel_rwx().is_user());
    assert!(!F::kernel_device().is_user());
}

/// RISC-V 地址边界编解码。
#[test]
fn riscv64_addr_boundary_roundtrip() {
    addr_boundary_roundtrip::<crate::riscv64::PageTableEntry>();
}

/// AArch64 地址边界编解码。
#[test]
fn aarch64_addr_boundary_roundtrip() {
    addr_boundary_roundtrip::<crate::aarch64::PageTableEntry>();
}

/// RISC-V 内核 preset 非用户态。
#[test]
fn riscv64_kernel_presets_not_user() {
    kernel_presets_not_user::<crate::riscv64::PteFlags>();
}

/// AArch64 内核 preset 非用户态。
#[test]
fn aarch64_kernel_presets_not_user() {
    kernel_presets_not_user::<crate::aarch64::PteFlags>();
}
