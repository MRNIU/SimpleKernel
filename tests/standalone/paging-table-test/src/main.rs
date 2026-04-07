//! 页表操作测试——验证 map/unmap/get_mapping/identity_map_range 等核心操作。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::{PhysAddr, VirtAddr};
use paging::error::PagingError;
use paging::{PageTable, PteFlags, PteFlagsOps, page_size_at_level};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_root_paddr_is_valid();
    log::info!("test root_paddr_is_valid ... ok");

    test_map_and_get_mapping();
    log::info!("test map_and_get_mapping ... ok");

    test_map_different_pages();
    log::info!("test map_different_pages ... ok");

    test_double_map_fails();
    log::info!("test double_map_fails ... ok");

    test_unmap_page_returns_old_pa();
    log::info!("test unmap_page_returns_old_pa ... ok");

    test_unmap_unmapped_page_fails();
    log::info!("test unmap_unmapped_page_fails ... ok");

    test_map_pages_in_different_vpn_ranges();
    log::info!("test map_pages_in_different_vpn_ranges ... ok");

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

    test_map_at_level1_huge_page();
    log::info!("test map_at_level1_huge_page ... ok");

    test_get_mapping_within_huge_page();
    log::info!("test get_mapping_within_huge_page ... ok");

    test_map_page_under_huge_page_fails();
    log::info!("test map_page_under_huge_page_fails ... ok");

    test_double_map_at_level_fails();
    log::info!("test double_map_at_level_fails ... ok");

    test_unmap_at_level1_huge_page();
    log::info!("test unmap_at_level1_huge_page ... ok");

    test_unmap_at_level_wrong_level_fails();
    log::info!("test unmap_at_level_wrong_level_fails ... ok");

    test_update_flags_changes_permissions();
    log::info!("test update_flags_changes_permissions ... ok");

    test_update_flags_unmapped_fails();
    log::info!("test update_flags_unmapped_fails ... ok");

    test_map_conflict_different_pa();
    log::info!("test map_conflict_different_pa ... ok");

    test_map_conflict_different_flags();
    log::info!("test map_conflict_different_flags ... ok");

    test_unmap_at_level_with_flags_returns_old_flags();
    log::info!("test unmap_at_level_with_flags_returns_old_flags ... ok");

    test_identity_map_range_auto_huge_page();
    log::info!("test identity_map_range_auto_huge_page ... ok");

    test_identity_map_range_idempotent();
    log::info!("test identity_map_range_idempotent ... ok");

    log::info!("paging-table-test: all 25 tests passed");
}

/// create 后 root_paddr 应返回非零地址。
fn test_root_paddr_is_valid() {
    let pt = PageTable::create().expect("创建测试页表失败");
    assert_ne!(pt.root_paddr().as_usize(), 0);
}

/// 映射单页后应能查询到正确的物理地址和完整标志。
fn test_map_and_get_mapping() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rw();

    pt.map_page(va, pa, flags).expect("map_page 应成功");

    let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能找到映射");
    assert_eq!(mapped_pa, pa);
    assert_eq!(mapped_flags, flags);
}

/// 映射两个不同的虚拟页到不同的物理页，互不干扰。
fn test_map_different_pages() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let va1 = VirtAddr::new(0x0000_1000);
    let va2 = VirtAddr::new(0x0000_2000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.map_page(va1, pa1, PteFlags::kernel_rw())
        .expect("map va1");
    pt.map_page(va2, pa2, PteFlags::kernel_rx())
        .expect("map va2");

    let (got_pa1, got_flags1) = pt.get_mapping(va1).expect("va1 应已映射");
    let (got_pa2, got_flags2) = pt.get_mapping(va2).expect("va2 应已映射");
    assert_eq!(got_pa1, pa1);
    assert_eq!(got_pa2, pa2);
    assert_eq!(got_flags1, PteFlags::kernel_rw());
    assert_eq!(got_flags2, PteFlags::kernel_rx());
}

/// 对同一虚拟地址重复映射应返回 AlreadyMapped 错误。
fn test_double_map_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_page(va, pa, PteFlags::kernel_rw())
        .expect("首次 map 应成功");
    let err = pt
        .map_page(va, pa, PteFlags::kernel_rw())
        .expect_err("重复 map 应失败");
    assert_eq!(err, PagingError::AlreadyMappedIdentical);
}

/// unmap 应返回原始物理地址，且之后查询应为 None。
fn test_unmap_page_returns_old_pa() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_page(va, pa, PteFlags::kernel_rw())
        .expect("map 应成功");
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

/// 跨不同 VPN[2] 范围的映射，会触发不同的二级页表分配。
fn test_map_pages_in_different_vpn_ranges() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let va_low = VirtAddr::new(0x0000_1000);
    let va_high = VirtAddr::new(0x4000_0000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.map_page(va_low, pa1, PteFlags::kernel_rw())
        .expect("map low");
    pt.map_page(va_high, pa2, PteFlags::kernel_rw())
        .expect("map high");

    let (got1, _) = pt.get_mapping(va_low).expect("low 应已映射");
    let (got2, _) = pt.get_mapping(va_high).expect("high 应已映射");
    assert_eq!(got1, pa1);
    assert_eq!(got2, pa2);
}

/// unmap 后重新映射到不同物理地址应成功。
fn test_remap_after_unmap() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8030_0000);

    pt.map_page(va, pa1, PteFlags::kernel_rw())
        .expect("首次 map");
    pt.unmap_page(va).expect("unmap");
    pt.map_page(va, pa2, PteFlags::kernel_rx())
        .expect("重映射应成功");

    let (got_pa, got_flags) = pt.get_mapping(va).expect("应找到新映射");
    assert_eq!(got_pa, pa2);
    assert_eq!(got_flags, PteFlags::kernel_rx());
}

/// 查询从未映射过的地址应返回 None。
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
    pt.map_page(va, pa, PteFlags::kernel_rw())
        .expect("map 应成功");
    pt.unmap_page(va).expect("unmap 应成功");

    // 中间节点已回收，同路径的兄弟 VA 也不可达
    let va_sibling = VirtAddr::new(0x2000);
    let err = pt
        .unmap_page(va_sibling)
        .expect_err("中间节点已回收，应返回 PageNotMapped");
    assert_eq!(err, PagingError::PageNotMapped);
}

/// unmap 后中间节点回收：当同表有其他映射时不回收。
fn test_unmap_preserves_intermediate_when_sibling_exists() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let va1 = VirtAddr::new(0x1000);
    let va2 = VirtAddr::new(0x2000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.map_page(va1, pa1, PteFlags::kernel_rw())
        .expect("map va1");
    pt.map_page(va2, pa2, PteFlags::kernel_rw())
        .expect("map va2");

    // unmap va1，va2 仍在同一中间节点中——中间节点不应被回收
    pt.unmap_page(va1).expect("unmap va1");
    assert!(pt.get_mapping(va1).is_none(), "va1 应已 unmap");
    assert!(pt.get_mapping(va2).is_some(), "va2 应仍然有效");

    // unmap va2 后可重新映射（中间节点此时回收，重新分配）
    pt.unmap_page(va2).expect("unmap va2");
    pt.map_page(va1, pa1, PteFlags::kernel_rw())
        .expect("重映射应成功");
}

/// identity_map_range 多页映射后应能逐页查询。
fn test_identity_map_range_multi_page() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let start = PhysAddr::new(0x10_0000);
    let end = PhysAddr::new(0x10_3000); // 3 pages

    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    for i in 0..3 {
        let va = VirtAddr::new(0x10_0000 + i * config::PAGE_SIZE);
        let (pa, _) = pt.get_mapping(va).expect("应能查到映射");
        assert_eq!(pa, PhysAddr::new(0x10_0000 + i * config::PAGE_SIZE));
    }
}

/// 大页映射：Level 1（2MB）应能映射和查询。
fn test_map_at_level1_huge_page() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let huge_size = page_size_at_level(1);
    let va = VirtAddr::new(huge_size);
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rw();

    pt.map_at_level(va, pa, flags, 1).expect("大页映射应成功");

    let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能查询到大页映射");
    assert_eq!(mapped_pa, pa);
    assert_eq!(mapped_flags, flags.for_leaf_at_level(1));
}

/// 大页范围内不同偏移处的 VA 应返回精确物理地址（基址 + 页内偏移）。
fn test_get_mapping_within_huge_page() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let huge_size = page_size_at_level(1);
    let va_base = VirtAddr::new(huge_size);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_at_level(va_base, pa, PteFlags::kernel_rw(), 1)
        .expect("大页映射应成功");

    // 基地址查询——偏移 0
    let (got_pa_base, _) = pt.get_mapping(va_base).expect("大页基地址应命中映射");
    assert_eq!(got_pa_base, pa);

    // 偏移地址查询——应返回 pa + 0x1000
    let va_offset = VirtAddr::new(huge_size + 0x1000);
    let (got_pa, _) = pt.get_mapping(va_offset).expect("大页内偏移地址应命中映射");
    assert_eq!(got_pa, pa + 0x1000);
}

/// 大页映射后，不可在同一路径上再映射子页（路径上遇到大页返回 HugePageConflict）。
fn test_map_page_under_huge_page_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let huge_size = page_size_at_level(1);
    let va = VirtAddr::new(huge_size);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
        .expect("大页映射应成功");

    let sub_va = VirtAddr::new(huge_size + 0x1000);
    let err = pt
        .map_page(sub_va, PhysAddr::new(0x9000_0000), PteFlags::kernel_rw())
        .expect_err("大页范围内的子映射应失败");
    assert_eq!(err, PagingError::HugePageConflict);
}

/// 重复大页映射应返回 AlreadyMapped。
fn test_double_map_at_level_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let huge_size = page_size_at_level(1);
    let va = VirtAddr::new(huge_size);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
        .expect("首次大页映射应成功");
    let err = pt
        .map_at_level(va, pa, PteFlags::kernel_rw(), 1)
        .expect_err("重复大页映射应失败");
    assert_eq!(err, PagingError::AlreadyMappedIdentical);
}

/// unmap_at_level_with_flags 应能取消大页映射。
fn test_unmap_at_level1_huge_page() {
    let mut pt = PageTable::create().expect("创建页表");
    let va = VirtAddr::new(0x0000_0000_4000_0000); // 1GB aligned
    let pa = PhysAddr::new(0x0000_0000_4000_0000);
    pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
        .expect("map level1");
    let (old_pa, _) = pt
        .unmap_at_level_with_flags(va, 1)
        .expect("unmap level1 应成功");
    assert_eq!(old_pa, pa);
    assert!(pt.get_mapping(va).is_none(), "unmap 后应无映射");
}

/// unmap_at_level_with_flags 目标层级无叶节点时应失败。
fn test_unmap_at_level_wrong_level_fails() {
    let mut pt = PageTable::create().expect("创建页表");
    let va = VirtAddr::new(0x0000_0000_4000_0000);
    let pa = PhysAddr::new(0x0000_0000_4000_0000);
    // 在 level 1 映射，尝试在 level 0 unmap 应失败
    pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
        .expect("map level1");
    let err = pt
        .unmap_at_level_with_flags(va, 0)
        .expect_err("level 0 unmap 大页应失败");
    assert_eq!(err, PagingError::PageNotMapped);
}

/// update_flags 应修改已映射页的权限并返回旧标志。
fn test_update_flags_changes_permissions() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_page(va, pa, PteFlags::kernel_rw())
        .expect("map 应成功");
    let old_flags = pt
        .update_flags(va, PteFlags::kernel_ro())
        .expect("update_flags 应成功");
    assert_eq!(old_flags, PteFlags::kernel_rw());

    let (got_pa, got_flags) = pt.get_mapping(va).expect("映射应存在");
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

/// 对同一 VA 映射不同 PA 应返回 AlreadyMappedConflict。
fn test_map_conflict_different_pa() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);

    pt.map_page(va, PhysAddr::new(0x8020_0000), PteFlags::kernel_rw())
        .expect("首次 map 应成功");
    let err = pt
        .map_page(va, PhysAddr::new(0x8030_0000), PteFlags::kernel_rw())
        .expect_err("不同 PA 重复 map 应失败");
    assert_eq!(err, PagingError::AlreadyMappedConflict);
}

/// 对同一 VA 映射不同 flags 应返回 AlreadyMappedConflict。
fn test_map_conflict_different_flags() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_page(va, pa, PteFlags::kernel_rw())
        .expect("首次 map 应成功");
    let err = pt
        .map_page(va, pa, PteFlags::kernel_ro())
        .expect_err("不同 flags 重复 map 应失败");
    assert_eq!(err, PagingError::AlreadyMappedConflict);
}

/// unmap_at_level_with_flags 应返回正确的旧 flags。
fn test_unmap_at_level_with_flags_returns_old_flags() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let huge_size = page_size_at_level(1);
    let va = VirtAddr::new(huge_size);
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rx();

    pt.map_at_level(va, pa, flags, 1).expect("大页映射应成功");
    let (old_pa, old_flags) = pt.unmap_at_level_with_flags(va, 1).expect("unmap 应成功");
    assert_eq!(old_pa, pa);
    assert_eq!(old_flags, flags.for_leaf_at_level(1));
}

/// identity_map_range 在对齐且足够大的区间应自动使用大页。
fn test_identity_map_range_auto_huge_page() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let huge_size = page_size_at_level(1); // 2MB
    let start = PhysAddr::new(huge_size);
    let end = PhysAddr::new(huge_size * 2);

    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    // 大页基址应能查询到映射
    let (pa, flags) = pt
        .get_mapping(VirtAddr::new(huge_size))
        .expect("大页基址应已映射");
    assert_eq!(pa, start);
    assert_eq!(flags, PteFlags::kernel_rw().for_leaf_at_level(1));

    // 大页内偏移地址也应命中
    let (pa_offset, _) = pt
        .get_mapping(VirtAddr::new(huge_size + 0x1000))
        .expect("大页内偏移应命中");
    assert_eq!(pa_offset, PhysAddr::new(huge_size + 0x1000));
}

/// identity_map_range 对相同 PA+flags 的重复映射应幂等（不 panic）。
fn test_identity_map_range_idempotent() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let start = PhysAddr::new(0x20_0000);
    let end = PhysAddr::new(0x20_2000); // 2 pages

    pt.identity_map_range(start, end, PteFlags::kernel_rw());
    // 重复映射相同区域——应幂等，不 panic
    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    let (pa, _) = pt
        .get_mapping(VirtAddr::new(0x20_0000))
        .expect("映射应存在");
    assert_eq!(pa, start);
}
