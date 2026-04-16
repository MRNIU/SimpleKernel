//! 页表操作测试——验证 identity_map_range / get_mapping 等公开操作。
//!
//! SAS 架构下，PageTable 只暴露 identity_map_range 和 get_mapping
//! 作为外部可用接口，map_page/unmap_page 等方法已收紧为 pub(crate)。

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

    test_get_mapping_on_empty_table();
    log::info!("test get_mapping_on_empty_table ... ok");

    test_identity_map_range_single_page();
    log::info!("test identity_map_range_single_page ... ok");

    test_identity_map_range_multi_page();
    log::info!("test identity_map_range_multi_page ... ok");

    test_identity_map_range_idempotent();
    log::info!("test identity_map_range_idempotent ... ok");

    test_identity_map_range_4kb_aligned_start();
    log::info!("test identity_map_range_4kb_aligned_start ... ok");

    test_identity_map_range_always_4kb();
    log::info!("test identity_map_range_always_4kb ... ok");

    log::info!("paging-table-test: all 7 tests passed");
}

/// create 后 root_paddr 应返回非零地址。
fn test_root_paddr_is_valid() {
    let pt = PageTable::create().expect("创建测试页表失败");
    assert_ne!(pt.root_paddr().as_usize(), 0);
}

/// 查询从未映射过的地址应返回 None。
fn test_get_mapping_on_empty_table() {
    let pt = PageTable::create().expect("创建测试页表失败");
    assert!(pt.get_mapping(VirtAddr::new(0x1000)).is_none());
    assert!(pt.get_mapping(VirtAddr::new(0)).is_none());
}

/// identity_map_range 单页映射后应能查询到正确的物理地址。
fn test_identity_map_range_single_page() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let start = PhysAddr::new(0x10_0000);
    let end = PhysAddr::new(0x10_1000);

    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    let (pa, _) = pt
        .get_mapping(VirtAddr::new(0x10_0000))
        .expect("应能查到映射");
    assert_eq!(pa, start);
}

/// identity_map_range 多页映射后应能逐页查询。
fn test_identity_map_range_multi_page() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let start = PhysAddr::new(0x10_0000);
    let end = PhysAddr::new(0x10_3000);

    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    for i in 0..3 {
        let va = VirtAddr::new(0x10_0000 + i * config::PAGE_SIZE);
        let (pa, _) = pt.get_mapping(va).expect("应能查到映射");
        assert_eq!(pa, PhysAddr::new(0x10_0000 + i * config::PAGE_SIZE));
    }
}

/// identity_map_range 对相同 PA+flags 的重复映射应幂等（不 panic）。
fn test_identity_map_range_idempotent() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let start = PhysAddr::new(0x20_0000);
    let end = PhysAddr::new(0x20_2000);

    pt.identity_map_range(start, end, PteFlags::kernel_rw());
    // 重复映射相同区域——应幂等，不 panic
    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    let (pa, _) = pt
        .get_mapping(VirtAddr::new(0x20_0000))
        .expect("映射应存在");
    assert_eq!(pa, start);
}

/// identity_map_range 对非页对齐起始地址应向下对齐。
fn test_identity_map_range_4kb_aligned_start() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    // 非对齐起始——应向下对齐到 0x10_0000
    let start = PhysAddr::new(0x10_0800);
    let end = PhysAddr::new(0x10_2000);

    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    // 对齐后的起始页 0x10_0000 应已映射
    let (pa_aligned, _) = pt
        .get_mapping(VirtAddr::new(0x10_0000))
        .expect("对齐起始页应已映射");
    assert_eq!(pa_aligned, PhysAddr::new(0x10_0000));
}

/// identity_map_range 统一使用 4KB 页——跨 2MB 对齐边界的区间也逐页映射。
fn test_identity_map_range_always_4kb() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let huge_size = paging::page_size_at_level(1); // 2MB
    // 2MB 对齐区间，4KB-only 路径下每页仍为 4KB 叶节点
    let start = PhysAddr::new(huge_size);
    let end = PhysAddr::new(huge_size + config::PAGE_SIZE * 3);

    pt.identity_map_range(start, end, PteFlags::kernel_rw());

    for i in 0..3 {
        let pa = PhysAddr::new(huge_size + i * config::PAGE_SIZE);
        let (got_pa, _) = pt
            .get_mapping(VirtAddr::new(pa.as_usize()))
            .expect("4KB 页应已映射");
        assert_eq!(got_pa, pa);
    }

    // 最后一页之后应无映射
    assert!(
        pt.get_mapping(VirtAddr::new(huge_size + 3 * config::PAGE_SIZE))
            .is_none()
    );
}
