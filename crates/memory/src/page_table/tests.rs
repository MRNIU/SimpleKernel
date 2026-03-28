//! 页表单元测试。

use super::test_table::TestPageTable;
use super::*;
use crate::address::{PhysAddr, VirtAddr};
use crate::error::MemoryError;

#[test]
/// PTE 编码往返测试：写入地址和标志，读回应一致。
fn pte_roundtrip() {
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PageFlags::VALID | PageFlags::READ | PageFlags::WRITE;
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa, "paddr round-trip failed");
    let recovered = pte.flags();
    assert!(recovered.contains(PageFlags::VALID));
    assert!(recovered.contains(PageFlags::READ));
    assert!(recovered.contains(PageFlags::WRITE));
}

#[test]
/// 空 PTE 应为 invalid 且非 leaf。
fn pte_empty_is_invalid() {
    let pte = PageTableEntry::empty();
    assert!(!pte.is_valid());
    assert!(!pte.is_leaf());
}

#[test]
/// 验证各预设标志组合的正确性。
fn page_flags_presets() {
    let rw = PageFlags::kernel_rw();
    assert!(rw.contains(PageFlags::VALID));
    assert!(rw.contains(PageFlags::READ));
    assert!(rw.contains(PageFlags::WRITE));
    assert!(rw.contains(PageFlags::GLOBAL));
    assert!(rw.contains(PageFlags::ACCESSED));
    assert!(rw.contains(PageFlags::DIRTY));
    assert!(!rw.contains(PageFlags::EXECUTE));

    let rx = PageFlags::kernel_rx();
    assert!(rx.contains(PageFlags::VALID));
    assert!(rx.contains(PageFlags::READ));
    assert!(rx.contains(PageFlags::EXECUTE));
    assert!(!rx.contains(PageFlags::WRITE));

    let rwx = PageFlags::kernel_rwx();
    assert!(rwx.contains(PageFlags::READ));
    assert!(rwx.contains(PageFlags::WRITE));
    assert!(rwx.contains(PageFlags::EXECUTE));
}

#[test]
/// 验证 PTE 的 valid 和 leaf 判断逻辑。
fn pte_is_valid_and_leaf() {
    let pa = PhysAddr::new(0x0000_1000);
    let leaf = PageTableEntry::new(pa, PageFlags::VALID | PageFlags::READ);
    assert!(leaf.is_valid());
    assert!(leaf.is_leaf());

    let intermediate = PageTableEntry::new(pa, PageFlags::VALID);
    assert!(intermediate.is_valid());
    assert!(!intermediate.is_leaf());
}

#[test]
/// 映射单页后应能查询到正确的物理地址和标志。
fn map_and_get_mapping() {
    let mut pt = TestPageTable::new();
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PageFlags::kernel_rw();

    pt.map_page(va, pa, flags).expect("map_page 应成功");

    let (mapped_pa, mapped_flags) = pt.get_mapping(va).expect("应能找到映射");
    assert_eq!(mapped_pa, pa);
    assert!(mapped_flags.contains(PageFlags::VALID));
    assert!(mapped_flags.contains(PageFlags::READ));
    assert!(mapped_flags.contains(PageFlags::WRITE));
}

#[test]
/// 映射两个不同的虚拟页到不同的物理页，互不干扰。
fn map_different_pages() {
    let mut pt = TestPageTable::new();

    let va1 = VirtAddr::new(0x0000_1000);
    let va2 = VirtAddr::new(0x0000_2000);
    let pa1 = PhysAddr::new(0x8020_0000);
    let pa2 = PhysAddr::new(0x8020_1000);

    pt.map_page(va1, pa1, PageFlags::kernel_rw())
        .expect("map va1");
    pt.map_page(va2, pa2, PageFlags::kernel_rx())
        .expect("map va2");

    let (got_pa1, _) = pt.get_mapping(va1).expect("va1 应已映射");
    let (got_pa2, _) = pt.get_mapping(va2).expect("va2 应已映射");
    assert_eq!(got_pa1, pa1);
    assert_eq!(got_pa2, pa2);
}

#[test]
/// 对同一虚拟地址重复映射应返回 MapFailed 错误。
fn double_map_fails() {
    let mut pt = TestPageTable::new();
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_page(va, pa, PageFlags::kernel_rw())
        .expect("首次 map 应成功");
    let err = pt
        .map_page(va, pa, PageFlags::kernel_rw())
        .expect_err("重复 map 应失败");
    assert_eq!(err, MemoryError::MapFailed);
}

#[test]
/// unmap 应返回原始物理地址，且之后查询应为 None。
fn unmap_page_returns_old_pa() {
    let mut pt = TestPageTable::new();
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_page(va, pa, PageFlags::kernel_rw())
        .expect("map 应成功");
    let old_pa = pt.unmap_page(va).expect("unmap 应成功");
    assert_eq!(old_pa, pa);

    // unmap 后再查询应为 None（PTE 已清零，is_leaf 为 false）
    assert!(pt.get_mapping(va).is_none());
}

#[test]
/// 对未映射的页执行 unmap 应返回 PageNotMapped 错误。
fn unmap_unmapped_page_fails() {
    let mut pt = TestPageTable::new();
    let va = VirtAddr::new(0x1000);

    let err = pt.unmap_page(va).expect_err("unmap 未映射页应失败");
    assert_eq!(err, MemoryError::PageNotMapped);
}

#[test]
/// 跨不同 VPN[2] 范围的映射，会触发不同的二级页表分配。
fn map_pages_in_different_vpn_ranges() {
    let mut pt = TestPageTable::new();

    let va_low = VirtAddr::new(0x0000_1000);
    let va_high = VirtAddr::new(0x4000_0000); // VPN[2] = 1
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
