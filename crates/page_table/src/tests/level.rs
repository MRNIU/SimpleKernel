//! 页表层级参数测试。

use crate::*;
use address::VirtAddr;

/// 验证 Level0 的 SHIFT 和 INDEX_BITS 从 PAGE_SIZE 正确推导。
#[test]
fn level0_params_match_page_size() {
    let page_shift = config::PAGE_SIZE.trailing_zeros() as usize;
    assert_eq!(Level0::SHIFT, page_shift);
    let pte_size_shift = core::mem::size_of::<PageTableEntry>().trailing_zeros() as usize;
    assert_eq!(Level0::INDEX_BITS, page_shift - pte_size_shift);
    assert_eq!(
        Level0::ENTRIES,
        config::PAGE_SIZE / core::mem::size_of::<PageTableEntry>()
    );
    assert_eq!(Level0::INDEX_MASK, Level0::ENTRIES - 1);
}

/// 验证各级 SHIFT 链式递增，且 INDEX_BITS 一致。
#[test]
fn level_shifts_are_chained() {
    assert_eq!(Level1::SHIFT, Level0::SHIFT + Level0::INDEX_BITS);
    assert_eq!(Level2::SHIFT, Level1::SHIFT + Level1::INDEX_BITS);
    assert_eq!(Level3::SHIFT, Level2::SHIFT + Level2::INDEX_BITS);
    assert_eq!(Level4::SHIFT, Level3::SHIFT + Level3::INDEX_BITS);
}

/// 验证 LEVEL_INFO 查表与 PageLevel trait 常量一致。
#[test]
fn level_info_matches_trait() {
    macro_rules! check_level {
        ($idx:expr, $L:ty) => {
            assert_eq!(LEVEL_INFO[$idx].shift, <$L>::SHIFT);
            assert_eq!(LEVEL_INFO[$idx].index_mask, <$L>::INDEX_MASK);
        };
    }
    check_level!(0, Level0);
    check_level!(1, Level1);
    check_level!(2, Level2);
    check_level!(3, Level3);
    check_level!(4, Level4);
}

/// 验证 page_size_at_level 返回正确的页大小。
#[test]
fn page_size_at_level_values() {
    assert_eq!(page_size_at_level(0), config::PAGE_SIZE);
    assert_eq!(page_size_at_level(1), 512 * config::PAGE_SIZE);
    assert_eq!(page_size_at_level(2), 512 * 512 * config::PAGE_SIZE);
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
