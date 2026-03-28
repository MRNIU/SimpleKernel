//! PTE 编解码测试——同时覆盖 RISC-V 和 AArch64。

use crate::{PteFlagsOps, PteOps};
use address::PhysAddr;

/// 地址编解码边界测试——验证零地址和高位地址不会污染标志字段。
///
/// 对两种架构的 PTE 均执行。
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

mod riscv64 {
    use super::*;
    use crate::riscv64::{PageTableEntry, PteFlags};

    /// 每个 RISC-V PteFlags 单独编解码往返。
    #[test]
    fn each_flag_roundtrip() {
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

    /// W^X 安全不变量：数据页不可执行，代码页不可写。
    #[test]
    fn wx_invariants() {
        let rw = PteFlags::kernel_rw();
        assert!(rw.is_writable());
        assert!(!rw.contains(PteFlags::EXECUTE));

        let rx = PteFlags::kernel_rx();
        assert!(!rx.is_writable());
        assert!(rx.contains(PteFlags::EXECUTE));

        let ro = PteFlags::kernel_ro();
        assert!(!ro.is_writable());
        assert!(!ro.contains(PteFlags::EXECUTE));

        let dev = PteFlags::kernel_device();
        assert!(!dev.contains(PteFlags::EXECUTE));
    }

    /// RISC-V 的 for_leaf_at_level 不改变标志位（格式与层级无关）。
    #[test]
    fn for_leaf_at_level_is_identity() {
        let flags = PteFlags::kernel_rw();
        assert_eq!(flags.for_leaf_at_level(0), flags);
        assert_eq!(flags.for_leaf_at_level(1), flags);
        assert_eq!(flags.for_leaf_at_level(2), flags);
    }

    /// EXCLUSIVE 软件位编解码往返。
    #[test]
    fn exclusive_roundtrip() {
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
    fn each_flag_roundtrip() {
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

    /// W^X 安全不变量：数据页 PXN + UXN，代码页仅 UXN，只读页 PXN + UXN。
    #[test]
    fn wx_invariants() {
        let rw = PteFlags::kernel_rw();
        assert!(rw.is_writable());
        assert!(rw.contains(PteFlags::PXN));
        assert!(rw.contains(PteFlags::UXN));

        let rx = PteFlags::kernel_rx();
        assert!(!rx.is_writable());
        assert!(!rx.contains(PteFlags::PXN));
        assert!(rx.contains(PteFlags::UXN));

        let ro = PteFlags::kernel_ro();
        assert!(!ro.is_writable());
        assert!(ro.contains(PteFlags::PXN));
        assert!(ro.contains(PteFlags::UXN));

        let dev = PteFlags::kernel_device();
        assert!(dev.contains(PteFlags::PXN));
        assert!(dev.contains(PteFlags::UXN));
    }

    /// AArch64 设备映射使用 MAIR_IDX1（Device-nGnRnE）。
    #[test]
    fn device_uses_mair_idx1() {
        let dev = PteFlags::kernel_device();
        assert!(dev.contains(PteFlags::MAIR_IDX1));
        assert!(!dev.contains(PteFlags::SH_INNER));
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

    /// EXCLUSIVE 软件位编解码往返。
    #[test]
    fn exclusive_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PteFlags::kernel_rw().with_exclusive();
        let pte = PageTableEntry::new(pa, flags);
        assert!(pte.flags().is_exclusive());
        assert_eq!(pte.paddr(), pa);

        let pte_no_excl = PageTableEntry::new(pa, PteFlags::kernel_rw());
        assert!(!pte_no_excl.flags().is_exclusive());
    }
}
