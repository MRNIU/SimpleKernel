//! 页表参数计算测试——验证 LEVEL_INFO、INDEX_BITS、vpn_index 的正确性。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::VirtAddr;
use paging::{
    ENTRIES_PER_TABLE, INDEX_BITS, INDEX_MASK, LEVEL_SHIFTS, page_size_at_level, vpn_index,
};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_level0_params_match_page_size();
    log::info!("test level0_params_match_page_size ... ok");

    test_level_shifts_are_chained();
    log::info!("test level_shifts_are_chained ... ok");

    test_page_size_at_level_values();
    log::info!("test page_size_at_level_values ... ok");

    test_vpn_index_extracts_correct_bits();
    log::info!("test vpn_index_extracts_correct_bits ... ok");

    log::info!("paging-basic-test: all 4 tests passed");
}

/// 验证 Level 0 的 shift 从 PAGE_SIZE 正确推导，INDEX_MASK 等于 ENTRIES_PER_TABLE - 1。
fn test_level0_params_match_page_size() {
    assert_eq!(LEVEL_SHIFTS[0], config::PAGE_SIZE_BITS);
    assert_eq!(INDEX_MASK, ENTRIES_PER_TABLE - 1);
}

/// 验证各级 shift 链式递增，步长为 INDEX_BITS。
fn test_level_shifts_are_chained() {
    for i in 1..LEVEL_SHIFTS.len() {
        assert_eq!(
            LEVEL_SHIFTS[i],
            LEVEL_SHIFTS[i - 1] + INDEX_BITS,
            "level {} shift 不正确",
            i
        );
    }
}

/// 验证 page_size_at_level 返回正确的页大小。
fn test_page_size_at_level_values() {
    assert_eq!(page_size_at_level(0), config::PAGE_SIZE);
    assert_eq!(page_size_at_level(1), ENTRIES_PER_TABLE * config::PAGE_SIZE);
    assert_eq!(
        page_size_at_level(2),
        ENTRIES_PER_TABLE * ENTRIES_PER_TABLE * config::PAGE_SIZE
    );
}

/// vpn_index 应正确提取各级索引。
fn test_vpn_index_extracts_correct_bits() {
    let va = VirtAddr::new(0x1000);
    assert_eq!(vpn_index(va, 0), 1);
    assert_eq!(vpn_index(va, 1), 0);
    assert_eq!(vpn_index(va, 2), 0);

    let va_high = VirtAddr::new(0x4000_0000);
    assert_eq!(vpn_index(va_high, 0), 0);
    assert_eq!(vpn_index(va_high, 1), 0);
    assert_eq!(vpn_index(va_high, 2), 1);
}
