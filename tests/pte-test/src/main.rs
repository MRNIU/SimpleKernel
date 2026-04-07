//! 页表项编解码测试——运行当前架构的 PTE 测试。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::PhysAddr;
use page_table_entry::{PageTableEntry, PteFlags, PteFlagsOps, PteOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Memory, run_tests);

fn run_tests() {
    test_each_flag_roundtrip();
    log::info!("test each_flag_roundtrip ... ok");

    test_wx_invariants();
    log::info!("test wx_invariants ... ok");

    test_user_presets();
    log::info!("test user_presets ... ok");

    test_user_wx_invariants();
    log::info!("test user_wx_invariants ... ok");

    test_invalid_pte_is_not_leaf();
    log::info!("test invalid_pte_is_not_leaf ... ok");

    test_intermediate_preserves_addr_and_is_not_leaf();
    log::info!("test intermediate_preserves_addr_and_is_not_leaf ... ok");

    test_addr_boundary_roundtrip();
    log::info!("test addr_boundary_roundtrip ... ok");

    test_kernel_presets_not_user();
    log::info!("test kernel_presets_not_user ... ok");

    #[cfg(target_arch = "riscv64")]
    {
        test_riscv64_for_leaf_at_level_is_identity();
        log::info!("test riscv64_for_leaf_at_level_is_identity ... ok");
    }

    #[cfg(target_arch = "aarch64")]
    {
        test_aarch64_device_uses_mair_idx1();
        log::info!("test aarch64_device_uses_mair_idx1 ... ok");

        test_aarch64_for_leaf_at_level_clears_table_bit();
        log::info!("test aarch64_for_leaf_at_level_clears_table_bit ... ok");

        test_aarch64_is_leaf_level_dependent();
        log::info!("test aarch64_is_leaf_level_dependent ... ok");
    }

    log::info!("pte-test: all tests passed");
}

/// 每个 PteFlags 单独编解码往返。
fn test_each_flag_roundtrip() {
    let pa = PhysAddr::new(0x8020_0000);

    #[cfg(target_arch = "riscv64")]
    {
        use page_table_entry::riscv64::PteFlags;
        let all_flags = [
            PteFlags::VALID,
            PteFlags::READ,
            PteFlags::READ | PteFlags::WRITE,
            PteFlags::EXECUTE,
            PteFlags::USER,
            PteFlags::GLOBAL,
            PteFlags::ACCESSED,
            PteFlags::DIRTY,
        ];
        for &flag in &all_flags {
            let pte = PageTableEntry::new(pa, flag);
            assert_eq!(pte.flags(), flag);
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        use page_table_entry::aarch64::PteFlags;
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
        ];
        for &flag in &all_flags {
            let pte = PageTableEntry::new(pa, flag);
            assert_eq!(pte.flags(), flag);
        }
    }
}

/// W^X 安全不变量。
fn test_wx_invariants() {
    let rw = PteFlags::kernel_rw();
    assert!(rw.is_writable());
    assert!(!rw.is_executable());

    let rx = PteFlags::kernel_rx();
    assert!(!rx.is_writable());
    assert!(rx.is_executable());

    let ro = PteFlags::kernel_ro();
    assert!(!ro.is_writable());
    assert!(!ro.is_executable());

    let rwx = PteFlags::kernel_rwx();
    assert!(rwx.is_writable());
    assert!(rwx.is_executable());

    let dev = PteFlags::kernel_device();
    assert!(!dev.is_executable());
}

/// 用户态 preset 设置了 user 位及架构特定标志。
fn test_user_presets() {
    assert!(PteFlags::user_rw().is_user());
    assert!(PteFlags::user_rx().is_user());
    assert!(PteFlags::user_ro().is_user());
    assert!(PteFlags::user_rwx().is_user());

    // RISC-V: 用户态 preset 不设 GLOBAL
    #[cfg(target_arch = "riscv64")]
    {
        use page_table_entry::riscv64::PteFlags;
        assert!(!PteFlags::user_rw().contains(PteFlags::GLOBAL));
        assert!(!PteFlags::user_rx().contains(PteFlags::GLOBAL));
        assert!(!PteFlags::user_ro().contains(PteFlags::GLOBAL));
        assert!(!PteFlags::user_rwx().contains(PteFlags::GLOBAL));
    }

    // AArch64: 用户态 preset 设 NG（Not-Global）
    #[cfg(target_arch = "aarch64")]
    {
        use page_table_entry::aarch64::PteFlags;
        assert!(PteFlags::user_rw().contains(PteFlags::NG));
        assert!(PteFlags::user_rx().contains(PteFlags::NG));
        assert!(PteFlags::user_ro().contains(PteFlags::NG));
        assert!(PteFlags::user_rwx().contains(PteFlags::NG));
    }
}

/// 用户态 preset 的 W^X 不变量。
fn test_user_wx_invariants() {
    let rw = PteFlags::user_rw();
    assert!(rw.is_writable());
    assert!(!rw.is_executable());

    let rx = PteFlags::user_rx();
    assert!(!rx.is_writable());
    assert!(rx.is_executable());

    let ro = PteFlags::user_ro();
    assert!(!ro.is_writable());
    assert!(!ro.is_executable());

    let rwx = PteFlags::user_rwx();
    assert!(rwx.is_writable());
    assert!(rwx.is_executable());
}

/// 无效 PTE 不应被视为叶节点。
fn test_invalid_pte_is_not_leaf() {
    let pte = PageTableEntry::from_raw(0);
    assert!(!pte.is_valid());
    assert!(!pte.is_leaf(0));
    assert!(!pte.is_leaf(1));
}

/// 中间节点 PTE 保留地址且不是叶节点。
fn test_intermediate_preserves_addr_and_is_not_leaf() {
    let pa = PhysAddr::new(0x8020_0000);
    let pte = PageTableEntry::new_intermediate(pa);
    assert!(pte.is_valid());
    assert_eq!(pte.paddr(), pa);

    #[cfg(target_arch = "riscv64")]
    {
        assert!(!pte.is_leaf(0));
        assert!(!pte.is_leaf(1));
    }
    #[cfg(target_arch = "aarch64")]
    {
        assert!(!pte.is_leaf(1));
        assert!(!pte.is_leaf(2));
    }
}

/// 地址编解码边界测试。
fn test_addr_boundary_roundtrip() {
    let flags = PteFlags::kernel_rw();

    let pa_zero = PhysAddr::new(0);
    let pte_zero = PageTableEntry::new(pa_zero, flags);
    assert_eq!(pte_zero.paddr(), pa_zero);
    assert_eq!(pte_zero.flags(), flags);

    let pa_high = PhysAddr::new(0x00FF_FFFF_F000);
    let pte_high = PageTableEntry::new(pa_high, flags);
    assert_eq!(pte_high.paddr(), pa_high);
    assert_eq!(pte_high.flags(), flags);
}

/// 内核 preset 不应被标记为用户态可访问。
fn test_kernel_presets_not_user() {
    assert!(!PteFlags::kernel_rw().is_user());
    assert!(!PteFlags::kernel_rx().is_user());
    assert!(!PteFlags::kernel_ro().is_user());
    assert!(!PteFlags::kernel_rwx().is_user());
    assert!(!PteFlags::kernel_device().is_user());
}

/// RISC-V: for_leaf_at_level 不改变标志位。
#[cfg(target_arch = "riscv64")]
fn test_riscv64_for_leaf_at_level_is_identity() {
    let flags = PteFlags::kernel_rw();
    assert_eq!(flags.for_leaf_at_level(0), flags);
    assert_eq!(flags.for_leaf_at_level(1), flags);
    assert_eq!(flags.for_leaf_at_level(2), flags);
}

/// AArch64: 设备映射使用 MAIR_IDX1。
#[cfg(target_arch = "aarch64")]
fn test_aarch64_device_uses_mair_idx1() {
    use page_table_entry::aarch64::PteFlags;
    let dev = PteFlags::kernel_device();
    assert!(dev.contains(PteFlags::MAIR_IDX1));
    assert!(!dev.contains(PteFlags::SH_INNER));
}

/// AArch64: for_leaf_at_level 在 Level > 0 清除 TABLE 位。
#[cfg(target_arch = "aarch64")]
fn test_aarch64_for_leaf_at_level_clears_table_bit() {
    use page_table_entry::aarch64::PteFlags;
    let flags = PteFlags::kernel_rw();
    assert!(flags.contains(PteFlags::TABLE));
    assert!(flags.for_leaf_at_level(0).contains(PteFlags::TABLE));
    assert!(!flags.for_leaf_at_level(1).contains(PteFlags::TABLE));
    assert!(!flags.for_leaf_at_level(2).contains(PteFlags::TABLE));
}

/// AArch64: is_leaf 依赖层级。
#[cfg(target_arch = "aarch64")]
fn test_aarch64_is_leaf_level_dependent() {
    let pa = PhysAddr::new(0x8020_0000);

    let page_pte = PageTableEntry::new(pa, PteFlags::kernel_rw());
    assert!(page_pte.is_leaf(0));

    let block_flags = PteFlags::kernel_rw().for_leaf_at_level(1);
    let block_pte = PageTableEntry::new(pa, block_flags);
    assert!(block_pte.is_leaf(1));

    let table_pte = PageTableEntry::new_intermediate(pa);
    assert!(!table_pte.is_leaf(1));
}
