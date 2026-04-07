//! VMA 测试——验证地址空间管理（mmap/munmap/find_vma/register_existing）。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use config::PAGE_SIZE;
use memory::error::MemoryError;
use memory::vma::AddressSpace;
use memory_types::VirtAddr;
use paging::{PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_empty_address_space();
    log::info!("test empty_address_space ... ok");

    test_mmap_creates_vma();
    log::info!("test mmap_creates_vma ... ok");

    test_find_vma_lookup();
    log::info!("test find_vma_lookup ... ok");

    test_munmap_removes_vma();
    log::info!("test munmap_removes_vma ... ok");

    test_mmap_zero_size_fails();
    log::info!("test mmap_zero_size_fails ... ok");

    test_register_existing_identical();
    log::info!("test register_existing_identical ... ok");

    test_register_existing_partial_overlap();
    log::info!("test register_existing_partial_overlap ... ok");

    test_register_existing_contained_overlap();
    log::info!("test register_existing_contained_overlap ... ok");

    log::info!("vma-test: all 8 tests passed");
}

/// 空地址空间应无 VMA。
fn test_empty_address_space() {
    let aspace = AddressSpace::new();
    assert_eq!(aspace.area_count(), 0);
    assert!(aspace.find_vma(VirtAddr::new(0x1000)).is_none());
}

/// mmap 应创建 VMA 并建立映射。
fn test_mmap_creates_vma() {
    let mut aspace = AddressSpace::new();
    let vma = aspace
        .mmap(PAGE_SIZE, PteFlags::kernel_rw())
        .expect("mmap 应成功");
    assert_eq!(vma.size(), PAGE_SIZE);
    assert!(vma.is_mapped());
    assert_eq!(aspace.area_count(), 1);
}

/// find_vma 应找到包含地址的 VMA。
fn test_find_vma_lookup() {
    let mut aspace = AddressSpace::new();
    let vma = aspace
        .mmap(3 * PAGE_SIZE, PteFlags::kernel_rw())
        .expect("mmap");
    let start = vma.start();
    assert!(aspace.find_vma(start).is_some());
    assert!(aspace.find_vma(start + PAGE_SIZE).is_some());
    assert!(aspace.find_vma(start + 3 * PAGE_SIZE).is_none());
}

/// munmap 应移除 VMA。
fn test_munmap_removes_vma() {
    let mut aspace = AddressSpace::new();
    let vma = aspace.mmap(PAGE_SIZE, PteFlags::kernel_rw()).expect("mmap");
    let start = vma.start();
    assert_eq!(aspace.area_count(), 1);
    aspace.munmap(start).expect("munmap 应成功");
    assert_eq!(aspace.area_count(), 0);
}

/// size 为 0 的 mmap 应失败。
fn test_mmap_zero_size_fails() {
    let mut aspace = AddressSpace::new();
    let err = aspace
        .mmap(0, PteFlags::kernel_rw())
        .expect_err("size=0 应失败");
    assert_eq!(err, MemoryError::MapFailed);
}

/// 完全相同的 register_existing 应返回 RegionIdentical。
fn test_register_existing_identical() {
    let mut aspace = AddressSpace::new();
    let start = VirtAddr::new(0x5000_0000);
    let size = PAGE_SIZE;

    aspace
        .register_existing(start, size, PteFlags::kernel_device())
        .expect("首次注册应成功");

    let err = aspace
        .register_existing(start, size, PteFlags::kernel_device())
        .expect_err("重复注册应返回错误");
    assert_eq!(err, MemoryError::RegionIdentical);
}

/// 部分重叠应返回 RegionOverlap。
fn test_register_existing_partial_overlap() {
    let mut aspace = AddressSpace::new();
    let start = VirtAddr::new(0x6000_0000);

    aspace
        .register_existing(start, 2 * PAGE_SIZE, PteFlags::kernel_device())
        .expect("首次注册应成功");

    // 后半部分重叠
    let err = aspace
        .register_existing(start + PAGE_SIZE, 2 * PAGE_SIZE, PteFlags::kernel_device())
        .expect_err("部分重叠应返回错误");
    assert_eq!(err, MemoryError::RegionOverlap);
}

/// 完全包含（新区域是已有区域的子集）应返回 RegionOverlap。
fn test_register_existing_contained_overlap() {
    let mut aspace = AddressSpace::new();
    let start = VirtAddr::new(0x7000_0000);

    aspace
        .register_existing(start, 4 * PAGE_SIZE, PteFlags::kernel_device())
        .expect("首次注册应成功");

    // 子集重叠
    let err = aspace
        .register_existing(start + PAGE_SIZE, PAGE_SIZE, PteFlags::kernel_device())
        .expect_err("包含关系应返回 RegionOverlap");
    assert_eq!(err, MemoryError::RegionOverlap);
}
