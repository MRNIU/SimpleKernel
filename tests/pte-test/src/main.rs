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

    test_kernel_preset_distinctness();
    log::info!("test kernel_preset_distinctness ... ok");

    test_kernel_firmware_preset();
    log::info!("test kernel_firmware_preset ... ok");

    test_invalid_pte_is_not_leaf();
    log::info!("test invalid_pte_is_not_leaf ... ok");

    test_intermediate_preserves_addr_and_is_not_leaf();
    log::info!("test intermediate_preserves_addr_and_is_not_leaf ... ok");

    test_addr_boundary_roundtrip();
    log::info!("test addr_boundary_roundtrip ... ok");

    #[cfg(target_arch = "riscv64")]
    {
        test_riscv64_for_leaf_at_level_is_identity();
        log::info!("test riscv64_for_leaf_at_level_is_identity ... ok");

        test_riscv64_wx_bits();
        log::info!("test riscv64_wx_bits ... ok");
    }

    #[cfg(target_arch = "aarch64")]
    {
        test_aarch64_device_uses_mair_idx1();
        log::info!("test aarch64_device_uses_mair_idx1 ... ok");

        test_aarch64_for_leaf_at_level_clears_table_bit();
        log::info!("test aarch64_for_leaf_at_level_clears_table_bit ... ok");

        test_aarch64_is_leaf_level_dependent();
        log::info!("test aarch64_is_leaf_level_dependent ... ok");

        test_aarch64_wx_bits();
        log::info!("test aarch64_wx_bits ... ok");
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
            PteFlags::AP_RO,
            PteFlags::SH_INNER,
            PteFlags::AF,
            PteFlags::PXN,
            PteFlags::UXN,
        ];
        for &flag in &all_flags {
            let pte = PageTableEntry::new(pa, flag);
            assert_eq!(pte.flags(), flag);
        }
    }
}

/// 常规内核 preset 应产生互不相同的 flags 位组合。
///
/// 这是对"工厂方法返回正确位"的烟测——kernel_rw/rx/ro 若被
/// 无意改同，调用方（`PageTable::update_range_flags`）会建立错误权限但不会立即报错。
fn test_kernel_preset_distinctness() {
    let rw = PteFlags::kernel_rw();
    let rx = PteFlags::kernel_rx();
    let ro = PteFlags::kernel_ro();

    assert_ne!(rw, rx);
    assert_ne!(rw, ro);
    assert_ne!(rx, ro);
}

/// 固件保留区 preset 显式复用背景层读写权限，不暴露通用 RWX preset。
fn test_kernel_firmware_preset() {
    assert_eq!(PteFlags::kernel_firmware(), PteFlags::kernel_rw());
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

/// RISC-V: for_leaf_at_level 不改变标志位。
#[cfg(target_arch = "riscv64")]
fn test_riscv64_for_leaf_at_level_is_identity() {
    let flags = PteFlags::kernel_rw();
    assert_eq!(flags.for_leaf_at_level(0), flags);
    assert_eq!(flags.for_leaf_at_level(1), flags);
    assert_eq!(flags.for_leaf_at_level(2), flags);
}

/// RISC-V: 内核 preset 的 R/W/X 位符合预期。
#[cfg(target_arch = "riscv64")]
fn test_riscv64_wx_bits() {
    use page_table_entry::riscv64::PteFlags;
    let rw = PteFlags::kernel_rw();
    assert!(rw.contains(PteFlags::READ | PteFlags::WRITE));
    assert!(!rw.contains(PteFlags::EXECUTE));

    let rx = PteFlags::kernel_rx();
    assert!(rx.contains(PteFlags::READ | PteFlags::EXECUTE));
    assert!(!rx.contains(PteFlags::WRITE));

    let ro = PteFlags::kernel_ro();
    assert!(ro.contains(PteFlags::READ));
    assert!(!ro.contains(PteFlags::WRITE | PteFlags::EXECUTE));
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

/// AArch64: 内核 preset 的 AP_RO/PXN/UXN 位符合预期。
#[cfg(target_arch = "aarch64")]
fn test_aarch64_wx_bits() {
    use page_table_entry::aarch64::PteFlags;
    // kernel_rw: 可写（无 AP_RO）、内核/用户均不可执行
    let rw = PteFlags::kernel_rw();
    assert!(!rw.contains(PteFlags::AP_RO));
    assert!(rw.contains(PteFlags::PXN | PteFlags::UXN));

    // kernel_rx: 只读（AP_RO）、可执行（无 PXN）、用户不可执行（UXN）
    let rx = PteFlags::kernel_rx();
    assert!(rx.contains(PteFlags::AP_RO));
    assert!(!rx.contains(PteFlags::PXN));
    assert!(rx.contains(PteFlags::UXN));

    // kernel_ro: 只读 + 均不可执行
    let ro = PteFlags::kernel_ro();
    assert!(ro.contains(PteFlags::AP_RO));
    assert!(ro.contains(PteFlags::PXN | PteFlags::UXN));
}
