//! 页表操作测试——验证 identity_map_range / get_mapping / update_pte 核心操作。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::{PhysAddr, VirtAddr};
use paging::{PageTable, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_root_paddr_is_valid();
    log::info!("test root_paddr_is_valid ... ok");

    test_identity_map_single_page();
    log::info!("test identity_map_single_page ... ok");

    test_identity_map_different_pages();
    log::info!("test identity_map_different_pages ... ok");

    test_identity_map_range_multi_page();
    log::info!("test identity_map_range_multi_page ... ok");

    test_identity_map_in_different_vpn_ranges();
    log::info!("test identity_map_in_different_vpn_ranges ... ok");

    test_get_mapping_on_empty_table();
    log::info!("test get_mapping_on_empty_table ... ok");

    test_update_pte_changes_permissions();
    log::info!("test update_pte_changes_permissions ... ok");

    test_identity_map_range_idempotent();
    log::info!("test identity_map_range_idempotent ... ok");

    test_update_range_flags_batch();
    log::info!("test update_range_flags_batch ... ok");

    log::info!("paging-table-test: all 9 tests passed");
}

/// create 后 root_paddr 应返回非零地址。
fn test_root_paddr_is_valid() {
    let pt = PageTable::create();
    assert_ne!(pt.root_paddr().as_usize(), 0);
}

/// 建立单页 identity map 后应能查询到。
fn test_identity_map_single_page() {
    let pt = PageTable::create();
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rw();

    pt.identity_map_range(pa, pa + config::PAGE_SIZE, flags);

    let (mapped_pa, mapped_flags) = pt.get_mapping(pa.to_virt()).expect("应能找到页表项");
    assert_eq!(mapped_pa, pa);
    assert_eq!(mapped_flags, flags);
}

/// 不同 PA 不同权限的页各自独立。
fn test_identity_map_different_pages() {
    let pt = PageTable::create();

    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.identity_map_range(pa1, pa1 + config::PAGE_SIZE, PteFlags::kernel_rw());
    pt.identity_map_range(pa2, pa2 + config::PAGE_SIZE, PteFlags::kernel_rx());

    let (got_pa1, got_flags1) = pt.get_mapping(pa1.to_virt()).expect("pa1 应有");
    let (got_pa2, got_flags2) = pt.get_mapping(pa2.to_virt()).expect("pa2 应有");
    assert_eq!(got_pa1, pa1);
    assert_eq!(got_pa2, pa2);
    assert_eq!(got_flags1, PteFlags::kernel_rw());
    assert_eq!(got_flags2, PteFlags::kernel_rx());
}

/// identity_map_range 多页后应能逐页查询。
fn test_identity_map_range_multi_page() {
    let pt = PageTable::create();
    let start = PhysAddr::new(0x10_0000);
    let end = PhysAddr::new(0x10_3000); // 3 pages

    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    for i in 0..3 {
        let va = VirtAddr::new(0x10_0000 + i * config::PAGE_SIZE);
        let (pa, _) = pt.get_mapping(va).expect("应能查到页表项");
        assert_eq!(pa, PhysAddr::new(0x10_0000 + i * config::PAGE_SIZE));
    }
}

/// 跨不同 VPN[2] 范围的操作，会触发不同的二级页表分配。
fn test_identity_map_in_different_vpn_ranges() {
    let pt = PageTable::create();

    let pa_low = PhysAddr::new(0x0000_1000);
    let pa_high = PhysAddr::new(0x4000_0000);

    pt.identity_map_range(pa_low, pa_low + config::PAGE_SIZE, PteFlags::kernel_rw());
    pt.identity_map_range(pa_high, pa_high + config::PAGE_SIZE, PteFlags::kernel_rw());

    let (got1, _) = pt.get_mapping(pa_low.to_virt()).expect("low 应有");
    let (got2, _) = pt.get_mapping(pa_high.to_virt()).expect("high 应有");
    assert_eq!(got1, pa_low);
    assert_eq!(got2, pa_high);
}

/// 查询从未操作过的地址应返回 None。
fn test_get_mapping_on_empty_table() {
    let pt = PageTable::create();
    assert!(pt.get_mapping(VirtAddr::new(0x1000)).is_none());
    assert!(pt.get_mapping(VirtAddr::new(0)).is_none());
}

/// update_pte 应修改已有页的权限。
fn test_update_pte_changes_permissions() {
    let pt = PageTable::create();
    let pa = PhysAddr::new(0x8020_0000);
    let va = pa.to_virt();

    pt.identity_map_range(pa, pa + config::PAGE_SIZE, PteFlags::kernel_rw());
    pt.update_pte(va, PteFlags::kernel_ro());

    let (got_pa, got_flags) = pt.get_mapping(va).expect("页表项应存在");
    assert_eq!(got_pa, pa);
    assert_eq!(got_flags, PteFlags::kernel_ro());
}

/// update_range_flags 应对整段连续页批量更新权限。
fn test_update_range_flags_batch() {
    let pt = PageTable::create();
    let start = PhysAddr::new(0x30_0000);
    let end = PhysAddr::new(0x30_3000); // 3 pages

    pt.identity_map_range(start, end, PteFlags::kernel_rw());
    pt.update_range_flags(start.to_virt(), 3, PteFlags::kernel_ro());

    for i in 0..3 {
        let va = VirtAddr::new(0x30_0000 + i * config::PAGE_SIZE);
        let (_, flags) = pt.get_mapping(va).expect("页表项应存在");
        assert_eq!(flags, PteFlags::kernel_ro(), "页 {i} 权限未更新");
    }
}

/// identity_map_range 对相同 PA+flags 的重复操作应幂等（不 panic）。
fn test_identity_map_range_idempotent() {
    let pt = PageTable::create();
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
