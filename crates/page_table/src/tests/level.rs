//! 页表层级参数测试。

use crate::*;
use address::VirtAddr;

/// 验证 Level 0 的参数从 PAGE_SIZE 正确推导。
#[test]
fn level0_params_match_page_size() {
    assert_eq!(LEVEL_INFO[0].shift, config::PAGE_SIZE_BITS);
    assert_eq!(LEVEL_INFO[0].index_mask, ENTRIES_PER_TABLE - 1);
}

/// 验证各级 shift 链式递增，步长为 INDEX_BITS。
#[test]
fn level_shifts_are_chained() {
    for i in 1..LEVEL_INFO.len() {
        assert_eq!(
            LEVEL_INFO[i].shift,
            LEVEL_INFO[i - 1].shift + INDEX_BITS,
            "level {} shift 不正确",
            i
        );
    }
}

/// 验证 page_size_at_level 返回正确的页大小。
#[test]
fn page_size_at_level_values() {
    assert_eq!(page_size_at_level(0), config::PAGE_SIZE);
    assert_eq!(page_size_at_level(1), ENTRIES_PER_TABLE * config::PAGE_SIZE);
    assert_eq!(
        page_size_at_level(2),
        ENTRIES_PER_TABLE * ENTRIES_PER_TABLE * config::PAGE_SIZE
    );
}

/// vpn_index 应正确提取各级索引。
#[test]
fn vpn_index_extracts_correct_bits() {
    let va = VirtAddr::new(0x1000);
    assert_eq!(vpn_index(va, 0), 1);
    assert_eq!(vpn_index(va, 1), 0);
    assert_eq!(vpn_index(va, 2), 0);

    let va_high = VirtAddr::new(0x4000_0000);
    assert_eq!(vpn_index(va_high, 0), 0);
    assert_eq!(vpn_index(va_high, 1), 0);
    assert_eq!(vpn_index(va_high, 2), 1);
}
