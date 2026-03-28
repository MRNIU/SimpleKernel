//! 页表单元测试。

use super::table::PageTable;
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

/// 验证 LEVEL_INFO 查表与 PageLevel trait 常量一致（shift 和 index_mask）。
#[test]
fn level_info_matches_trait() {
    assert_eq!(LEVEL_INFO[0].shift, Level0::SHIFT);
    assert_eq!(LEVEL_INFO[0].index_mask, Level0::INDEX_MASK);
    assert_eq!(LEVEL_INFO[1].shift, Level1::SHIFT);
    assert_eq!(LEVEL_INFO[1].index_mask, Level1::INDEX_MASK);
    assert_eq!(LEVEL_INFO[2].shift, Level2::SHIFT);
    assert_eq!(LEVEL_INFO[2].index_mask, Level2::INDEX_MASK);
    assert_eq!(LEVEL_INFO[3].shift, Level3::SHIFT);
    assert_eq!(LEVEL_INFO[3].index_mask, Level3::INDEX_MASK);
    assert_eq!(LEVEL_INFO[4].shift, Level4::SHIFT);
    assert_eq!(LEVEL_INFO[4].index_mask, Level4::INDEX_MASK);
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
    let flags = PteFlags::VALID | PteFlags::READ | PteFlags::WRITE;
    let pte = PageTableEntry::new(pa, flags);

    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
}

/// PTE 往返测试——零地址，验证地址和标志均正确。
#[test]
fn pte_roundtrip_zero_addr() {
    let pa = PhysAddr::new(0);
    let flags = PteFlags::VALID | PteFlags::READ;
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
}

/// PTE 往返测试——高位地址，验证地址高位不会污染标志字段。
#[test]
fn pte_roundtrip_high_addr() {
    let pa = PhysAddr::new(0x00FF_FFFF_F000);
    let flags = PteFlags::VALID | PteFlags::READ | PteFlags::WRITE;
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
}

/// 每个 PteFlags 单独编解码往返，确保各 bit 互不干扰。
#[test]
fn pte_each_flag_roundtrip() {
    let pa = PhysAddr::new(0x8020_0000);
    let all_flags = [
        PteFlags::VALID,
        PteFlags::READ,
        PteFlags::WRITE,
        PteFlags::EXECUTE,
        PteFlags::USER,
        PteFlags::GLOBAL,
        PteFlags::ACCESSED,
        PteFlags::DIRTY,
    ];
    for &flag in &all_flags {
        let pte = PageTableEntry::new(pa, flag);
        assert_eq!(pte.flags(), flag, "标志 {:?} 编解码往返失败", flag);
    }
}

/// 空 PTE 应为 invalid 且非 leaf。
#[test]
fn pte_empty_is_invalid() {
    let pte = PageTableEntry::empty();
    assert!(!pte.is_valid());
    assert!(!pte.is_leaf());
}

/// 验证各预设标志组合的正确性（RISC-V Sv39 位布局）。
#[test]
fn pte_flags_presets() {
    let rw = PteFlags::kernel_rw();
    assert_eq!(
        rw,
        PteFlags::VALID
            | PteFlags::READ
            | PteFlags::WRITE
            | PteFlags::GLOBAL
            | PteFlags::ACCESSED
            | PteFlags::DIRTY
    );

    let rx = PteFlags::kernel_rx();
    assert_eq!(
        rx,
        PteFlags::VALID
            | PteFlags::READ
            | PteFlags::EXECUTE
            | PteFlags::GLOBAL
            | PteFlags::ACCESSED
    );

    let ro = PteFlags::kernel_ro();
    assert_eq!(
        ro,
        PteFlags::VALID | PteFlags::READ | PteFlags::GLOBAL | PteFlags::ACCESSED
    );

    let rwx = PteFlags::kernel_rwx();
    assert_eq!(
        rwx,
        PteFlags::VALID
            | PteFlags::READ
            | PteFlags::WRITE
            | PteFlags::EXECUTE
            | PteFlags::GLOBAL
            | PteFlags::ACCESSED
            | PteFlags::DIRTY
    );
}

/// 验证 `is_writable()` 语义方法在各预设下的正确性。
#[test]
fn pte_flags_is_writable() {
    assert!(PteFlags::kernel_rw().is_writable());
    assert!(PteFlags::kernel_rwx().is_writable());
    assert!(!PteFlags::kernel_rx().is_writable());
    assert!(!PteFlags::kernel_ro().is_writable());
    assert!(PteFlags::WRITE.is_writable());
    assert!(!PteFlags::empty().is_writable());
}

/// 验证 PTE 的 valid 和 leaf 判断逻辑。
#[test]
fn pte_is_valid_and_leaf() {
    let pa = PhysAddr::new(0x0000_1000);
    let leaf = PageTableEntry::new(pa, PteFlags::VALID | PteFlags::READ);
    assert!(leaf.is_valid());
    assert!(leaf.is_leaf());

    let intermediate = PageTableEntry::new(pa, PteFlags::VALID);
    assert!(intermediate.is_valid());
    assert!(!intermediate.is_leaf());
}

/// 映射单页后应能查询到正确的物理地址和完整标志。
#[test]
fn map_and_get_mapping() {
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
#[test]
fn map_different_pages() {
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

/// 对同一虚拟地址重复映射应返回 MapFailed 错误。
#[test]
fn double_map_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_page(va, pa, PteFlags::kernel_rw())
        .expect("首次 map 应成功");
    let err = pt
        .map_page(va, pa, PteFlags::kernel_rw())
        .expect_err("重复 map 应失败");
    assert_eq!(err, MemoryError::MapFailed);
}

/// unmap 应返回原始物理地址，且之后查询应为 None。
#[test]
fn unmap_page_returns_old_pa() {
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
#[test]
fn unmap_unmapped_page_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);

    let err = pt.unmap_page(va).expect_err("unmap 未映射页应失败");
    assert_eq!(err, MemoryError::PageNotMapped);
}

/// 跨不同 VPN[2] 范围的映射，会触发不同的二级页表分配。
#[test]
fn map_pages_in_different_vpn_ranges() {
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
#[test]
fn remap_after_unmap() {
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
#[test]
fn get_mapping_on_empty_table() {
    let pt = PageTable::create().expect("创建测试页表失败");
    assert!(pt.get_mapping(VirtAddr::new(0x1000)).is_none());
    assert!(pt.get_mapping(VirtAddr::new(0)).is_none());
}

/// 中间节点 PTE 应标记为 valid 但非 leaf。
#[test]
fn intermediate_pte_roundtrip() {
    let pa = PhysAddr::new(0x8020_0000);
    let pte = PageTableEntry::new_intermediate(pa);
    assert!(pte.is_valid());
    assert!(!pte.is_leaf());
    assert_eq!(pte.paddr(), pa);
}

/// 中间节点存在但叶 PTE 为空时，unmap 应返回 PageNotMapped。
///
/// 与 `unmap_unmapped_page_fails`（完全空表）走不同的 find_pte_mut 路径：
/// 前者在中间层级即返回 None，本测试在叶级发现 PTE 无效。
#[test]
fn unmap_empty_leaf_with_existing_intermediate() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    // 先映射再 unmap，留下中间节点但叶 PTE 为空
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);
    pt.map_page(va, pa, PteFlags::kernel_rw())
        .expect("map 应成功");
    pt.unmap_page(va).expect("unmap 应成功");

    // 同一 VPN 路径下的不同叶 PTE（共享中间节点）
    let va_same_path = VirtAddr::new(0x2000);
    let err = pt
        .unmap_page(va_same_path)
        .expect_err("叶 PTE 为空，应返回 PageNotMapped");
    assert_eq!(err, MemoryError::PageNotMapped);
}
