//! 页表单元测试。

use super::table::TestPageTable;
use super::*;
use crate::error::MemoryError;
use address::{PhysAddr, VirtAddr};

/// 验证 Level0 的 SHIFT 和 INDEX_BITS 从 PAGE_SIZE 正确推导。
#[test]
fn level0_params_match_page_size() {
    let page_shift = config::PAGE_SIZE.trailing_zeros() as usize;
    assert_eq!(Level0::SHIFT, page_shift);
    assert_eq!(Level0::INDEX_BITS, page_shift - 3);
    assert_eq!(Level0::ENTRIES, config::PAGE_SIZE / 8);
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
    assert_eq!(LEVEL_INFO[0].shift, Level0::SHIFT);
    assert_eq!(LEVEL_INFO[0].index_mask, Level0::INDEX_MASK);
    assert_eq!(LEVEL_INFO[1].shift, Level1::SHIFT);
    assert_eq!(LEVEL_INFO[2].shift, Level2::SHIFT);
    assert_eq!(LEVEL_INFO[3].shift, Level3::SHIFT);
    assert_eq!(LEVEL_INFO[4].shift, Level4::SHIFT);
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

/// PTE 编码往返测试：写入地址和标志，读回应一致。
#[test]
fn pte_roundtrip() {
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PageFlags::VALID | PageFlags::READ | PageFlags::WRITE;
    let pte = PageTableEntry::new(pa, flags);

    assert_eq!(pte.paddr(), pa);
    let recovered = pte.flags();
    assert!(recovered.contains(PageFlags::VALID));
    assert!(recovered.contains(PageFlags::READ));
    assert!(recovered.contains(PageFlags::WRITE));
    assert!(!recovered.contains(PageFlags::EXECUTE));
}

/// PTE 往返测试——零地址。
#[test]
fn pte_roundtrip_zero_addr() {
    let pa = PhysAddr::new(0);
    let flags = PageFlags::VALID | PageFlags::READ;
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
}

/// PTE 往返测试——高位地址。
#[test]
fn pte_roundtrip_high_addr() {
    let pa = PhysAddr::new(0x00FF_FFFF_F000);
    let flags = PageFlags::VALID | PageFlags::READ | PageFlags::WRITE;
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
}

/// 空 PTE 应为 invalid 且非 leaf。
#[test]
fn pte_empty_is_invalid() {
    let pte = PageTableEntry::empty();
    assert!(!pte.is_valid());
    assert!(!pte.is_leaf());
}

/// 验证各预设标志组合的正确性。
#[test]
fn page_flags_presets() {
    let rw = PageFlags::kernel_rw();
    assert!(rw.contains(PageFlags::VALID));
    assert!(rw.contains(PageFlags::READ));
    assert!(rw.contains(PageFlags::WRITE));
    assert!(rw.contains(PageFlags::GLOBAL));
    assert!(rw.contains(PageFlags::ACCESSED));
    assert!(rw.contains(PageFlags::DIRTY));
    assert!(!rw.contains(PageFlags::EXECUTE));
    assert!(!rw.contains(PageFlags::USER));

    let rx = PageFlags::kernel_rx();
    assert!(rx.contains(PageFlags::VALID));
    assert!(rx.contains(PageFlags::READ));
    assert!(rx.contains(PageFlags::EXECUTE));
    assert!(!rx.contains(PageFlags::WRITE));
    assert!(!rx.contains(PageFlags::DIRTY));

    let ro = PageFlags::kernel_ro();
    assert!(ro.contains(PageFlags::READ));
    assert!(!ro.contains(PageFlags::WRITE));
    assert!(!ro.contains(PageFlags::EXECUTE));

    let rwx = PageFlags::kernel_rwx();
    assert!(rwx.contains(PageFlags::READ));
    assert!(rwx.contains(PageFlags::WRITE));
    assert!(rwx.contains(PageFlags::EXECUTE));
}

/// 验证 PTE 的 valid 和 leaf 判断逻辑。
#[test]
fn pte_is_valid_and_leaf() {
    let pa = PhysAddr::new(0x0000_1000);
    let leaf = PageTableEntry::new(pa, PageFlags::VALID | PageFlags::READ);
    assert!(leaf.is_valid());
    assert!(leaf.is_leaf());

    let intermediate = PageTableEntry::new(pa, PageFlags::VALID);
    assert!(intermediate.is_valid());
    assert!(!intermediate.is_leaf());
}

/// 映射单页后应能查询到正确的物理地址和完整标志。
#[test]
fn map_and_get_mapping() {
    let mut pt = TestPageTable::create();
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PageFlags::kernel_rw();

    pt.map_page(va, pa, flags).expect("map_page 应成功");

    let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能找到映射");
    assert_eq!(mapped_pa, pa);
    assert_eq!(mapped_flags, flags);
}

/// 映射两个不同的虚拟页到不同的物理页，互不干扰。
#[test]
fn map_different_pages() {
    let mut pt = TestPageTable::create();

    let va1 = VirtAddr::new(0x0000_1000);
    let va2 = VirtAddr::new(0x0000_2000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.map_page(va1, pa1, PageFlags::kernel_rw())
        .expect("map va1");
    pt.map_page(va2, pa2, PageFlags::kernel_rx())
        .expect("map va2");

    let (got_pa1, got_flags1) = pt.get_mapping(va1).expect("va1 应已映射");
    let (got_pa2, got_flags2) = pt.get_mapping(va2).expect("va2 应已映射");
    assert_eq!(got_pa1, pa1);
    assert_eq!(got_pa2, pa2);
    assert_eq!(got_flags1, PageFlags::kernel_rw());
    assert_eq!(got_flags2, PageFlags::kernel_rx());
}

/// 对同一虚拟地址重复映射应返回 MapFailed 错误。
#[test]
fn double_map_fails() {
    let mut pt = TestPageTable::create();
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_page(va, pa, PageFlags::kernel_rw())
        .expect("首次 map 应成功");
    let err = pt
        .map_page(va, pa, PageFlags::kernel_rw())
        .expect_err("重复 map 应失败");
    assert_eq!(err, MemoryError::MapFailed);
}

/// unmap 应返回原始物理地址，且之后查询应为 None。
#[test]
fn unmap_page_returns_old_pa() {
    let mut pt = TestPageTable::create();
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_page(va, pa, PageFlags::kernel_rw())
        .expect("map 应成功");
    let old_pa = pt.unmap_page(va).expect("unmap 应成功");
    assert_eq!(old_pa, pa);

    assert!(pt.get_mapping(va).is_none());
}

/// 对未映射的页执行 unmap 应返回 PageNotMapped 错误。
#[test]
fn unmap_unmapped_page_fails() {
    let mut pt = TestPageTable::create();
    let va = VirtAddr::new(0x1000);

    let err = pt.unmap_page(va).expect_err("unmap 未映射页应失败");
    assert_eq!(err, MemoryError::PageNotMapped);
}

/// 跨不同 VPN[2] 范围的映射，会触发不同的二级页表分配。
#[test]
fn map_pages_in_different_vpn_ranges() {
    let mut pt = TestPageTable::create();

    let va_low = VirtAddr::new(0x0000_1000);
    let va_high = VirtAddr::new(0x4000_0000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.map_page(va_low, pa1, PageFlags::kernel_rw())
        .expect("map low");
    pt.map_page(va_high, pa2, PageFlags::kernel_rw())
        .expect("map high");

    let (got1, _) = pt.get_mapping(va_low).expect("low 应已映射");
    let (got2, _) = pt.get_mapping(va_high).expect("high 应已映射");
    assert_eq!(got1, pa1);
    assert_eq!(got2, pa2);
}

/// unmap 后重新映射到不同物理地址应成功。
#[test]
fn remap_after_unmap() {
    let mut pt = TestPageTable::create();
    let va = VirtAddr::new(0x1000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8030_0000);

    pt.map_page(va, pa1, PageFlags::kernel_rw())
        .expect("首次 map");
    pt.unmap_page(va).expect("unmap");
    pt.map_page(va, pa2, PageFlags::kernel_rx())
        .expect("重映射应成功");

    let (got_pa, got_flags) = pt.get_mapping(va).expect("应找到新映射");
    assert_eq!(got_pa, pa2);
    assert_eq!(got_flags, PageFlags::kernel_rx());
}

/// 查询从未映射过的地址应返回 None。
#[test]
fn get_mapping_on_empty_table() {
    let pt = TestPageTable::create();
    assert!(pt.get_mapping(VirtAddr::new(0x1000)).is_none());
    assert!(pt.get_mapping(VirtAddr::new(0)).is_none());
}
