//! 页表操作测试——验证 create_pte/get_mapping/update_pte/identity_map_range 核心操作。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::{PhysAddr, VirtAddr};
use paging::error::PagingError;
use paging::{PageTable, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_root_paddr_is_valid();
    log::info!("test root_paddr_is_valid ... ok");

    test_create_pte_and_get_mapping();
    log::info!("test create_pte_and_get_mapping ... ok");

    test_create_pte_different_pages();
    log::info!("test create_pte_different_pages ... ok");

    test_create_pte_idempotent();
    log::info!("test create_pte_idempotent ... ok");

    test_create_pte_in_different_vpn_ranges();
    log::info!("test create_pte_in_different_vpn_ranges ... ok");

    test_get_mapping_on_empty_table();
    log::info!("test get_mapping_on_empty_table ... ok");

    test_identity_map_range_multi_page();
    log::info!("test identity_map_range_multi_page ... ok");

    test_update_pte_changes_permissions();
    log::info!("test update_pte_changes_permissions ... ok");

    test_update_pte_unmapped_fails();
    log::info!("test update_pte_unmapped_fails ... ok");

    test_create_pte_flags_conflict();
    log::info!("test create_pte_flags_conflict ... ok");

    test_identity_map_range_idempotent();
    log::info!("test identity_map_range_idempotent ... ok");

    log::info!("paging-table-test: all 11 tests passed");
}

/// create 后 root_paddr 应返回非零地址。
fn test_root_paddr_is_valid() {
    let pt = PageTable::create().expect("创建测试页表失败");
    assert_ne!(pt.root_paddr().as_usize(), 0);
}

/// 设置单页权限后应能查询到正确的物理地址和标志。
fn test_create_pte_and_get_mapping() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rw();

    pt.create_pte(va, pa, flags).expect("create_pte 应成功");

    let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能找到页表项");
    assert_eq!(mapped_pa, pa);
    assert_eq!(mapped_flags, flags);
}

/// 为两个不同的虚拟页设置不同权限，互不干扰。
fn test_create_pte_different_pages() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let va1 = VirtAddr::new(0x0000_1000);
    let va2 = VirtAddr::new(0x0000_2000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.create_pte(va1, pa1, PteFlags::kernel_rw())
        .expect("set va1");
    pt.create_pte(va2, pa2, PteFlags::kernel_rx())
        .expect("set va2");

    let (got_pa1, got_flags1) = pt.get_mapping(va1).expect("va1 应有页表项");
    let (got_pa2, got_flags2) = pt.get_mapping(va2).expect("va2 应有页表项");
    assert_eq!(got_pa1, pa1);
    assert_eq!(got_pa2, pa2);
    assert_eq!(got_flags1, PteFlags::kernel_rw());
    assert_eq!(got_flags2, PteFlags::kernel_rx());
}

/// 同 VA + 同 PA + 同 flags 的重复调用应幂等。
fn test_create_pte_idempotent() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.create_pte(va, pa, PteFlags::kernel_rw())
        .expect("首次 create 应成功");
    pt.create_pte(va, pa, PteFlags::kernel_rw())
        .expect("幂等 create 应成功");
}

/// 跨不同 VPN[2] 范围的操作，会触发不同的二级页表分配。
fn test_create_pte_in_different_vpn_ranges() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let va_low = VirtAddr::new(0x0000_1000);
    let va_high = VirtAddr::new(0x4000_0000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.create_pte(va_low, pa1, PteFlags::kernel_rw())
        .expect("set low");
    pt.create_pte(va_high, pa2, PteFlags::kernel_rw())
        .expect("set high");

    let (got1, _) = pt.get_mapping(va_low).expect("low 应有页表项");
    let (got2, _) = pt.get_mapping(va_high).expect("high 应有页表项");
    assert_eq!(got1, pa1);
    assert_eq!(got2, pa2);
}

/// 查询从未操作过的地址应返回 None。
fn test_get_mapping_on_empty_table() {
    let pt = PageTable::create().expect("创建测试页表失败");
    assert!(pt.get_mapping(VirtAddr::new(0x1000)).is_none());
    assert!(pt.get_mapping(VirtAddr::new(0)).is_none());
}

/// identity_map_range 多页后应能逐页查询。
fn test_identity_map_range_multi_page() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let start = PhysAddr::new(0x10_0000);
    let end = PhysAddr::new(0x10_3000); // 3 pages

    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    for i in 0..3 {
        let va = VirtAddr::new(0x10_0000 + i * config::PAGE_SIZE);
        let (pa, _) = pt.get_mapping(va).expect("应能查到页表项");
        assert_eq!(pa, PhysAddr::new(0x10_0000 + i * config::PAGE_SIZE));
    }
}

/// update_pte 应修改已有页的权限并返回旧标志。
fn test_update_pte_changes_permissions() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.create_pte(va, pa, PteFlags::kernel_rw())
        .expect("create 应成功");
    let old_flags = pt
        .update_pte(va, PteFlags::kernel_ro())
        .expect("update_pte 应成功");
    assert_eq!(old_flags, PteFlags::kernel_rw());

    let (got_pa, got_flags) = pt.get_mapping(va).expect("页表项应存在");
    assert_eq!(got_pa, pa);
    assert_eq!(got_flags, PteFlags::kernel_ro());
}

/// update_pte 对未映射页应返回 PageNotMapped。
fn test_update_pte_unmapped_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let err = pt
        .update_pte(VirtAddr::new(0x1000), PteFlags::kernel_ro())
        .expect_err("未映射页 update_pte 应失败");
    assert_eq!(err, PagingError::PageNotMapped);
}

/// 同 VA + 同 PA + 不同 flags → FlagsConflict 错误（显式修改请用 update_pte）。
fn test_create_pte_flags_conflict() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.create_pte(va, pa, PteFlags::kernel_rw())
        .expect("首次 create 应成功");
    let err = pt
        .create_pte(va, pa, PteFlags::kernel_ro())
        .expect_err("不同 flags 应返回 FlagsConflict");
    assert_eq!(err, PagingError::FlagsConflict);
}

/// identity_map_range 对相同 PA+flags 的重复操作应幂等（不 panic）。
fn test_identity_map_range_idempotent() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let start = PhysAddr::new(0x20_0000);
    let end = PhysAddr::new(0x20_2000); // 2 pages

    pt.identity_map_range(start, end, PteFlags::kernel_rw());
    // 重复操作相同区域——应幂等，不 panic
    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    let (pa, _) = pt
        .get_mapping(VirtAddr::new(0x20_0000))
        .expect("页表项应存在");
    assert_eq!(pa, start);
}
