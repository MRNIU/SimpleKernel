//! PTE 编解码测试——同时覆盖 RISC-V 和 AArch64。

use crate::{PteFlagsOps, PteOps};
use address::PhysAddr;

/// PTE_SIZE_SHIFT 应等于 log2(8) = 3（64 位 PTE）。
#[test]
fn pte_size_shift_matches_u64() {
    assert_eq!(crate::PTE_SIZE_SHIFT, 3);
    assert_eq!(1usize << crate::PTE_SIZE_SHIFT, core::mem::size_of::<u64>());
}

/// 架构无关的 PTE trait 一致性测试——对任意 PteOps 实现进行验证。
fn check_pte_ops<T: PteOps>()
where
    T::Flags: PartialEq,
{
    let pa = PhysAddr::new(0x8020_0000);

    // 叶 PTE 往返
    let flags = T::Flags::kernel_rw();
    let pte = T::new(pa, flags);
    assert!(pte.is_valid());
    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
    assert!(pte.is_leaf(0));

    // 空 PTE
    let empty = T::empty();
    assert!(!empty.is_valid());
    assert!(!empty.is_leaf(0));

    // 中间节点 PTE
    let inter = T::new_intermediate(pa);
    assert!(inter.is_valid());
    assert!(!inter.is_leaf(1));
    assert_eq!(inter.paddr(), pa);

    // 零地址往返
    let pa_zero = PhysAddr::new(0);
    let pte_zero = T::new(pa_zero, T::Flags::kernel_ro());
    assert_eq!(pte_zero.paddr(), pa_zero);
    assert_eq!(pte_zero.flags(), T::Flags::kernel_ro());

    // 高位地址往返
    let pa_high = PhysAddr::new(0x00FF_FFFF_F000);
    let pte_high = T::new(pa_high, T::Flags::kernel_rw());
    assert_eq!(pte_high.paddr(), pa_high);
    assert_eq!(pte_high.flags(), T::Flags::kernel_rw());
}

/// 架构无关的 PteFlagsOps 一致性测试。
fn check_flags_ops<T: PteFlagsOps>() {
    assert!(T::kernel_rw().is_writable());
    assert!(!T::kernel_rx().is_writable());
    assert!(!T::kernel_ro().is_writable());
    assert!(T::kernel_rwx().is_writable());
    assert!(T::kernel_device().is_writable());

    let flags = T::kernel_rw();
    assert_eq!(
        flags.for_leaf_at_level(0).is_writable(),
        flags.is_writable()
    );

    // EXCLUSIVE 位
    assert!(!flags.is_exclusive());
    let exclusive = flags.with_exclusive();
    assert!(exclusive.is_exclusive());
    assert!(exclusive.is_writable());
}

/// RISC-V PTE trait 一致性。
#[test]
fn riscv64_pte_ops_conformance() {
    check_pte_ops::<crate::riscv64::PageTableEntry>();
}

/// AArch64 PTE trait 一致性。
#[test]
fn aarch64_pte_ops_conformance() {
    check_pte_ops::<crate::aarch64::PageTableEntry>();
}

/// RISC-V PteFlags trait 一致性。
#[test]
fn riscv64_flags_ops_conformance() {
    check_flags_ops::<crate::riscv64::PteFlags>();
}

/// AArch64 PteFlags trait 一致性。
#[test]
fn aarch64_flags_ops_conformance() {
    check_flags_ops::<crate::aarch64::PteFlags>();
}

mod riscv64 {
    use super::*;
    use crate::riscv64::{PageTableEntry, PteFlags};

    /// 每个 RISC-V PteFlags 单独编解码往返。
    #[test]
    fn pte_each_flag_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let all_flags = [
            PteFlags::VALID,
            PteFlags::READ,
            PteFlags::WRITE,
            PteFlags::EXECUTE,
            PteFlags::USER,
            PteFlags::GLOBAL,
            PteFlags::ACCESSED,
            PteFlags::DIRTY,
            PteFlags::EXCLUSIVE,
        ];
        for &flag in &all_flags {
            let pte = PageTableEntry::new(pa, flag);
            assert_eq!(pte.flags(), flag, "标志 {:?} 编解码往返失败", flag);
        }
    }

    /// 验证 RISC-V 各预设标志组合的正确性。
    #[test]
    fn pte_flags_presets() {
        let rw = PteFlags::kernel_rw();
        assert_eq!(
            rw,
            PteFlags::VALID
                | PteFlags::READ
                | PteFlags::WRITE
                | PteFlags::GLOBAL
                | PteFlags::ACCESSED
                | PteFlags::DIRTY
        );

        let rx = PteFlags::kernel_rx();
        assert_eq!(
            rx,
            PteFlags::VALID
                | PteFlags::READ
                | PteFlags::EXECUTE
                | PteFlags::GLOBAL
                | PteFlags::ACCESSED
        );

        let ro = PteFlags::kernel_ro();
        assert_eq!(
            ro,
            PteFlags::VALID | PteFlags::READ | PteFlags::GLOBAL | PteFlags::ACCESSED
        );

        let rwx = PteFlags::kernel_rwx();
        assert_eq!(
            rwx,
            PteFlags::VALID
                | PteFlags::READ
                | PteFlags::WRITE
                | PteFlags::EXECUTE
                | PteFlags::GLOBAL
                | PteFlags::ACCESSED
                | PteFlags::DIRTY
        );

        // kernel_device 与 kernel_rw 相同（RISC-V 无页表级缓存属性）
        assert_eq!(PteFlags::kernel_device(), PteFlags::kernel_rw());
    }

    /// RISC-V 的 for_leaf_at_level 不改变标志位。
    #[test]
    fn for_leaf_at_level_is_identity() {
        let flags = PteFlags::kernel_rw();
        assert_eq!(flags.for_leaf_at_level(0), flags);
        assert_eq!(flags.for_leaf_at_level(1), flags);
        assert_eq!(flags.for_leaf_at_level(2), flags);
    }

    /// EXCLUSIVE 位编解码往返。
    #[test]
    fn exclusive_flag_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PteFlags::kernel_rw().with_exclusive();
        let pte = PageTableEntry::new(pa, flags);
        assert!(pte.flags().is_exclusive());
        assert_eq!(pte.paddr(), pa);

        let pte_no_excl = PageTableEntry::new(pa, PteFlags::kernel_rw());
        assert!(!pte_no_excl.flags().is_exclusive());
    }
}

mod aarch64 {
    use super::*;
    use crate::aarch64::{PageTableEntry, PteFlags};

    /// 每个 AArch64 PteFlags 单独编解码往返。
    #[test]
    fn pte_each_flag_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let all_flags = [
            PteFlags::VALID,
            PteFlags::TABLE,
            PteFlags::MAIR_IDX1,
            PteFlags::AP_UNPRIV,
            PteFlags::AP_RO,
            PteFlags::SH_INNER,
            PteFlags::AF,
            PteFlags::NG,
            PteFlags::PXN,
            PteFlags::UXN,
            PteFlags::EXCLUSIVE,
        ];
        for &flag in &all_flags {
            let pte = PageTableEntry::new(pa, flag);
            assert_eq!(pte.flags(), flag, "标志 {:?} 编解码往返失败", flag);
        }
    }

    /// 验证 AArch64 各预设标志组合的正确性。
    #[test]
    fn pte_flags_presets() {
        let rw = PteFlags::kernel_rw();
        assert!(rw.is_writable());
        assert!(rw.contains(PteFlags::VALID));
        assert!(rw.contains(PteFlags::AF));
        assert!(rw.contains(PteFlags::PXN));
        assert!(rw.contains(PteFlags::UXN));
        assert!(!rw.contains(PteFlags::AP_RO));

        let rx = PteFlags::kernel_rx();
        assert!(!rx.is_writable());
        assert!(rx.contains(PteFlags::AP_RO));
        assert!(!rx.contains(PteFlags::PXN));

        let ro = PteFlags::kernel_ro();
        assert!(!ro.is_writable());
        assert!(ro.contains(PteFlags::PXN));
        assert!(ro.contains(PteFlags::UXN));
    }

    /// AArch64 的 for_leaf_at_level：Level 0 保留 TABLE 位，Level > 0 清除。
    #[test]
    fn for_leaf_at_level_clears_table_bit() {
        let flags = PteFlags::kernel_rw();
        assert!(flags.contains(PteFlags::TABLE));
        assert!(flags.for_leaf_at_level(0).contains(PteFlags::TABLE));
        assert!(!flags.for_leaf_at_level(1).contains(PteFlags::TABLE));
        assert!(!flags.for_leaf_at_level(2).contains(PteFlags::TABLE));
    }

    /// AArch64 设备映射使用 MAIR_IDX1。
    #[test]
    fn kernel_device_uses_mair_idx1() {
        let dev = PteFlags::kernel_device();
        assert!(dev.contains(PteFlags::MAIR_IDX1));
        assert!(dev.contains(PteFlags::PXN));
        assert!(dev.contains(PteFlags::UXN));
    }

    /// AArch64 的 is_leaf 依赖层级。
    #[test]
    fn is_leaf_level_dependent() {
        let pa = PhysAddr::new(0x8020_0000);

        let page_pte = PageTableEntry::new(pa, PteFlags::kernel_rw());
        assert!(page_pte.is_leaf(0));

        let block_flags = PteFlags::kernel_rw().for_leaf_at_level(1);
        let block_pte = PageTableEntry::new(pa, block_flags);
        assert!(block_pte.is_leaf(1));

        let table_pte = PageTableEntry::new_intermediate(pa);
        assert!(!table_pte.is_leaf(1));
    }

    /// EXCLUSIVE 位编解码往返。
    #[test]
    fn exclusive_flag_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PteFlags::kernel_rw().with_exclusive();
        let pte = PageTableEntry::new(pa, flags);
        assert!(pte.flags().is_exclusive());
        assert_eq!(pte.paddr(), pa);

        let pte_no_excl = PageTableEntry::new(pa, PteFlags::kernel_rw());
        assert!(!pte_no_excl.flags().is_exclusive());
    }
}
