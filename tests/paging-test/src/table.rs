//! 页表操作测试——验证 set_page_flags/unmap/get_mapping/identity_map_range 等核心操作。

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

    test_set_page_flags_and_get_mapping();
    log::info!("test set_page_flags_and_get_mapping ... ok");

    test_set_page_flags_different_pages();
    log::info!("test set_page_flags_different_pages ... ok");

    test_set_page_flags_idempotent();
    log::info!("test set_page_flags_idempotent ... ok");

    test_unmap_page_returns_old_pa();
    log::info!("test unmap_page_returns_old_pa ... ok");

    test_unmap_unmapped_page_fails();
    log::info!("test unmap_unmapped_page_fails ... ok");

    test_set_page_flags_in_different_vpn_ranges();
    log::info!("test set_page_flags_in_different_vpn_ranges ... ok");

    test_remap_after_unmap();
    log::info!("test remap_after_unmap ... ok");

    test_get_mapping_on_empty_table();
    log::info!("test get_mapping_on_empty_table ... ok");

    test_unmap_reclaims_intermediate_then_sibling_fails();
    log::info!("test unmap_reclaims_intermediate_then_sibling_fails ... ok");

    test_unmap_preserves_intermediate_when_sibling_exists();
    log::info!("test unmap_preserves_intermediate_when_sibling_exists ... ok");

    test_identity_map_range_multi_page();
    log::info!("test identity_map_range_multi_page ... ok");

    test_update_flags_changes_permissions();
    log::info!("test update_flags_changes_permissions ... ok");

    test_update_flags_unmapped_fails();
    log::info!("test update_flags_unmapped_fails ... ok");

    test_set_page_flags_updates_flags_same_pa();
    log::info!("test set_page_flags_updates_flags_same_pa ... ok");

    test_unmap_page_with_flags_returns_old_flags();
    log::info!("test unmap_page_with_flags_returns_old_flags ... ok");

    test_identity_map_range_idempotent();
    log::info!("test identity_map_range_idempotent ... ok");

    log::info!("paging-table-test: all 17 tests passed");
}

/// create 后 root_paddr 应返回非零地址。
fn test_root_paddr_is_valid() {
    let pt = PageTable::create().expect("创建测试页表失败");
    assert_ne!(pt.root_paddr().as_usize(), 0);
}

/// 设置单页权限后应能查询到正确的物理地址和标志。
fn test_set_page_flags_and_get_mapping() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rw();

    pt.set_page_flags(va, pa, flags)
        .expect("set_page_flags 应成功");

    let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能找到页表项");
    assert_eq!(mapped_pa, pa);
    assert_eq!(mapped_flags, flags);
}

/// 为两个不同的虚拟页设置不同权限，互不干扰。
fn test_set_page_flags_different_pages() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let va1 = VirtAddr::new(0x0000_1000);
    let va2 = VirtAddr::new(0x0000_2000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.set_page_flags(va1, pa1, PteFlags::kernel_rw())
        .expect("set va1");
    pt.set_page_flags(va2, pa2, PteFlags::kernel_rx())
        .expect("set va2");

    let (got_pa1, got_flags1) = pt.get_mapping(va1).expect("va1 应有页表项");
    let (got_pa2, got_flags2) = pt.get_mapping(va2).expect("va2 应有页表项");
    assert_eq!(got_pa1, pa1);
    assert_eq!(got_pa2, pa2);
    assert_eq!(got_flags1, PteFlags::kernel_rw());
    assert_eq!(got_flags2, PteFlags::kernel_rx());
}

/// 同 VA + 同 PA + 同 flags 的重复调用应幂等。
fn test_set_page_flags_idempotent() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.set_page_flags(va, pa, PteFlags::kernel_rw())
        .expect("首次 set 应成功");
    pt.set_page_flags(va, pa, PteFlags::kernel_rw())
        .expect("幂等 set 应成功");
}

/// unmap 应返回原始物理地址，且之后查询应为 None。
fn test_unmap_page_returns_old_pa() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.set_page_flags(va, pa, PteFlags::kernel_rw())
        .expect("set 应成功");
    let old_pa = pt.unmap_page(va).expect("unmap 应成功");
    assert_eq!(old_pa, pa);

    assert!(pt.get_mapping(va).is_none());
}

/// 对未映射的页执行 unmap 应返回 PageNotMapped 错误。
fn test_unmap_unmapped_page_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);

    let err = pt.unmap_page(va).expect_err("unmap 未映射页应失败");
    assert_eq!(err, PagingError::PageNotMapped);
}

/// 跨不同 VPN[2] 范围的操作，会触发不同的二级页表分配。
fn test_set_page_flags_in_different_vpn_ranges() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let va_low = VirtAddr::new(0x0000_1000);
    let va_high = VirtAddr::new(0x4000_0000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.set_page_flags(va_low, pa1, PteFlags::kernel_rw())
        .expect("set low");
    pt.set_page_flags(va_high, pa2, PteFlags::kernel_rw())
        .expect("set high");

    let (got1, _) = pt.get_mapping(va_low).expect("low 应有页表项");
    let (got2, _) = pt.get_mapping(va_high).expect("high 应有页表项");
    assert_eq!(got1, pa1);
    assert_eq!(got2, pa2);
}

/// unmap 后重新设置到不同物理地址应成功。
fn test_remap_after_unmap() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8030_0000);

    pt.set_page_flags(va, pa1, PteFlags::kernel_rw())
        .expect("首次 set");
    pt.unmap_page(va).expect("unmap");
    pt.set_page_flags(va, pa2, PteFlags::kernel_rx())
        .expect("重新 set 应成功");

    let (got_pa, got_flags) = pt.get_mapping(va).expect("应找到新页表项");
    assert_eq!(got_pa, pa2);
    assert_eq!(got_flags, PteFlags::kernel_rx());
}

/// 查询从未操作过的地址应返回 None。
fn test_get_mapping_on_empty_table() {
    let pt = PageTable::create().expect("创建测试页表失败");
    assert!(pt.get_mapping(VirtAddr::new(0x1000)).is_none());
    assert!(pt.get_mapping(VirtAddr::new(0)).is_none());
}

/// unmap 唯一叶后，中间节点也被回收——同路径上的其他 VA unmap 应返回 PageNotMapped。
fn test_unmap_reclaims_intermediate_then_sibling_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);
    pt.set_page_flags(va, pa, PteFlags::kernel_rw())
        .expect("set 应成功");
    pt.unmap_page(va).expect("unmap 应成功");

    // 中间节点已回收，同路径的兄弟 VA 也不可达
    let va_sibling = VirtAddr::new(0x2000);
    let err = pt
        .unmap_page(va_sibling)
        .expect_err("中间节点已回收，应返回 PageNotMapped");
    assert_eq!(err, PagingError::PageNotMapped);
}

/// unmap 后中间节点回收：当同表有其他页表项时不回收。
fn test_unmap_preserves_intermediate_when_sibling_exists() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let va1 = VirtAddr::new(0x1000);
    let va2 = VirtAddr::new(0x2000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.set_page_flags(va1, pa1, PteFlags::kernel_rw())
        .expect("set va1");
    pt.set_page_flags(va2, pa2, PteFlags::kernel_rw())
        .expect("set va2");

    // unmap va1，va2 仍在同一中间节点中——中间节点不应被回收
    pt.unmap_page(va1).expect("unmap va1");
    assert!(pt.get_mapping(va1).is_none(), "va1 应已 unmap");
    assert!(pt.get_mapping(va2).is_some(), "va2 应仍然有效");

    // unmap va2 后可重新操作（中间节点此时回收，重新分配）
    pt.unmap_page(va2).expect("unmap va2");
    pt.set_page_flags(va1, pa1, PteFlags::kernel_rw())
        .expect("重新 set 应成功");
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

/// update_flags 应修改已有页的权限并返回旧标志。
fn test_update_flags_changes_permissions() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.set_page_flags(va, pa, PteFlags::kernel_rw())
        .expect("set 应成功");
    let old_flags = pt
        .update_flags(va, PteFlags::kernel_ro())
        .expect("update_flags 应成功");
    assert_eq!(old_flags, PteFlags::kernel_rw());

    let (got_pa, got_flags) = pt.get_mapping(va).expect("页表项应存在");
    assert_eq!(got_pa, pa);
    assert_eq!(got_flags, PteFlags::kernel_ro());
}

/// update_flags 对未映射页应返回 PageNotMapped。
fn test_update_flags_unmapped_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let err = pt
        .update_flags(VirtAddr::new(0x1000), PteFlags::kernel_ro())
        .expect_err("未映射页 update_flags 应失败");
    assert_eq!(err, PagingError::PageNotMapped);
}

/// 同 VA + 同 PA + 不同 flags → 更新权限。
fn test_set_page_flags_updates_flags_same_pa() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.set_page_flags(va, pa, PteFlags::kernel_rw())
        .expect("首次 set 应成功");
    pt.set_page_flags(va, pa, PteFlags::kernel_ro())
        .expect("更新权限应成功");

    let (_, flags) = pt.get_mapping(va).expect("页表项应存在");
    assert_eq!(flags, PteFlags::kernel_ro());
}

/// unmap_page_with_flags 应返回正确的旧 flags。
fn test_unmap_page_with_flags_returns_old_flags() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rx();

    pt.set_page_flags(va, pa, flags).expect("set 应成功");
    let (old_pa, old_flags) = pt.unmap_page_with_flags(va).expect("unmap 应成功");
    assert_eq!(old_pa, pa);
    assert_eq!(old_flags, flags);
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
