//! PTE 编解码测试。

use crate::*;
use address::PhysAddr;

/// PTE_SIZE_SHIFT 应等于 log2(8) = 3（64 位 PTE）。
#[test]
fn pte_size_shift_matches_u64() {
    assert_eq!(PTE_SIZE_SHIFT, 3);
    assert_eq!(
        1usize << PTE_SIZE_SHIFT,
        core::mem::size_of::<PageTableEntry>()
    );
}

/// PTE 编码往返测试：通过 preset 写入，读回应一致。
#[test]
fn pte_roundtrip_via_preset() {
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rw();
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
}

/// PTE 往返测试——零地址。
#[test]
fn pte_roundtrip_zero_addr() {
    let pa = PhysAddr::new(0);
    let flags = PteFlags::kernel_ro();
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
}

/// PTE 往返测试——高位地址，验证地址高位不会污染标志字段。
#[test]
fn pte_roundtrip_high_addr() {
    let pa = PhysAddr::new(0x00FF_FFFF_F000);
    let flags = PteFlags::kernel_rw();
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
}

/// 空 PTE 应为 invalid 且非 leaf。
#[test]
fn pte_empty_is_invalid() {
    let pte = PageTableEntry::empty();
    assert!(!pte.is_valid());
    assert!(!pte.is_leaf(0));
}

/// 中间节点 PTE 应标记为 valid 但非 leaf。
#[test]
fn intermediate_pte_roundtrip() {
    let pa = PhysAddr::new(0x8020_0000);
    let pte = PageTableEntry::new_intermediate(pa);
    assert!(pte.is_valid());
    assert!(!pte.is_leaf(1));
    assert_eq!(pte.paddr(), pa);
}

/// 叶 PTE（通过 kernel_rw）应为 valid 且 leaf。
#[test]
fn leaf_pte_is_valid_and_leaf() {
    let pa = PhysAddr::new(0x0000_1000);
    let pte = PageTableEntry::new(pa, PteFlags::kernel_rw());
    assert!(pte.is_valid());
    assert!(pte.is_leaf(0));
}

/// 验证 PteFlagsOps trait 所有方法在 PteFlags 上的可用性。
#[test]
fn pte_flags_trait_conformance() {
    fn check<T: PteFlagsOps>() {
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
        assert!(!flags.is_exclusive());
        let exclusive = flags.with_exclusive();
        assert!(exclusive.is_exclusive());
        assert!(exclusive.is_writable());
    }
    check::<PteFlags>();
}

/// EXCLUSIVE 位编解码往返——通过 PTE 写入再读出后 EXCLUSIVE 位应保留。
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

/// 验证 PteOps trait 所有方法在 PageTableEntry 上的可用性。
#[test]
fn pte_ops_trait_conformance() {
    fn check<T: PteOps>() {
        let pa = PhysAddr::new(0x8020_0000);
        let pte = T::new(pa, T::Flags::kernel_rw());
        assert!(pte.is_valid());
        assert_eq!(pte.paddr(), pa);

        let empty = T::empty();
        assert!(!empty.is_valid());

        let inter = T::new_intermediate(pa);
        assert!(inter.is_valid());
        assert!(!inter.is_leaf(1));
    }
    check::<PageTableEntry>();
}

#[cfg(not(feature = "test-aarch64"))]
mod riscv64_specific {
    use super::*;

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
}

#[cfg(feature = "test-aarch64")]
mod aarch64_specific {
    use super::*;

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

    /// AArch64 的 is_leaf 依赖层级：Level 0 所有有效项都是叶，Level > 0 看 TABLE 位。
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
}
