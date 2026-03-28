//! 页表单元测试。

use crate::error::PageTableError;
use crate::*;
use address::{PhysAddr, VirtAddr};

use crate::HeapNodeFrame;
type PageTable = crate::table::PageTable<HeapNodeFrame>;

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
    assert_eq!(LEVEL_INFO[1].index_mask, Level1::INDEX_MASK);
    assert_eq!(LEVEL_INFO[2].shift, Level2::SHIFT);
    assert_eq!(LEVEL_INFO[2].index_mask, Level2::INDEX_MASK);
    assert_eq!(LEVEL_INFO[3].shift, Level3::SHIFT);
    assert_eq!(LEVEL_INFO[3].index_mask, Level3::INDEX_MASK);
    assert_eq!(LEVEL_INFO[4].shift, Level4::SHIFT);
    assert_eq!(LEVEL_INFO[4].index_mask, Level4::INDEX_MASK);
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

/// PTE 编码往返测试：通过 preset 写入，读回应一致。
#[test]
fn pte_roundtrip_via_preset() {
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rw();
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
}

/// PTE 往返测试——零地址。
#[test]
fn pte_roundtrip_zero_addr() {
    let pa = PhysAddr::new(0);
    let flags = PteFlags::kernel_ro();
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
}

/// PTE 往返测试——高位地址，验证地址高位不会污染标志字段。
#[test]
fn pte_roundtrip_high_addr() {
    let pa = PhysAddr::new(0x00FF_FFFF_F000);
    let flags = PteFlags::kernel_rw();
    let pte = PageTableEntry::new(pa, flags);
    assert_eq!(pte.paddr(), pa);
    assert_eq!(pte.flags(), flags);
}

/// 空 PTE 应为 invalid 且非 leaf。
#[test]
fn pte_empty_is_invalid() {
    let pte = PageTableEntry::empty();
    assert!(!pte.is_valid());
    assert!(!pte.is_leaf(0));
}

/// 中间节点 PTE 应标记为 valid 但非 leaf。
#[test]
fn intermediate_pte_roundtrip() {
    let pa = PhysAddr::new(0x8020_0000);
    let pte = PageTableEntry::new_intermediate(pa);
    assert!(pte.is_valid());
    assert!(!pte.is_leaf(1));
    assert_eq!(pte.paddr(), pa);
}

/// 叶 PTE（通过 kernel_rw）应为 valid 且 leaf。
#[test]
fn leaf_pte_is_valid_and_leaf() {
    let pa = PhysAddr::new(0x0000_1000);
    let pte = PageTableEntry::new(pa, PteFlags::kernel_rw());
    assert!(pte.is_valid());
    assert!(pte.is_leaf(0));
}

/// 验证 PteFlagsOps trait 所有方法在 PteFlags 上的可用性。
#[test]
fn pte_flags_trait_conformance() {
    fn check<T: PteFlagsOps>() {
        assert!(T::kernel_rw().is_writable());
        assert!(!T::kernel_rx().is_writable());
        assert!(!T::kernel_ro().is_writable());
        assert!(T::kernel_rwx().is_writable());
        assert!(T::kernel_device().is_writable());
        let flags = T::kernel_rw();
        assert_eq!(
            flags.for_leaf_at_level(0).is_writable(),
            flags.is_writable()
        );
        // EXCLUSIVE 位
        assert!(!flags.is_exclusive());
        let exclusive = flags.with_exclusive();
        assert!(exclusive.is_exclusive());
        assert!(exclusive.is_writable());
    }
    check::<PteFlags>();
}

/// EXCLUSIVE 位编解码往返——通过 PTE 写入再读出后 EXCLUSIVE 位应保留。
#[test]
fn exclusive_flag_roundtrip() {
    let pa = PhysAddr::new(0x8020_0000);
    let flags = PteFlags::kernel_rw().with_exclusive();
    let pte = PageTableEntry::new(pa, flags);
    assert!(pte.flags().is_exclusive());
    assert_eq!(pte.paddr(), pa);

    let pte_no_excl = PageTableEntry::new(pa, PteFlags::kernel_rw());
    assert!(!pte_no_excl.flags().is_exclusive());
}

/// 验证 PteOps trait 所有方法在 PageTableEntry 上的可用性。
#[test]
fn pte_ops_trait_conformance() {
    fn check<T: PteOps>() {
        let pa = PhysAddr::new(0x8020_0000);
        let pte = T::new(pa, T::Flags::kernel_rw());
        assert!(pte.is_valid());
        assert_eq!(pte.paddr(), pa);

        let empty = T::empty();
        assert!(!empty.is_valid());

        let inter = T::new_intermediate(pa);
        assert!(inter.is_valid());
        assert!(!inter.is_leaf(1));
    }
    check::<PageTableEntry>();
}

#[cfg(not(feature = "test-aarch64"))]
mod riscv64_specific {
    use super::*;

    /// 每个 RISC-V PteFlags 单独编解码往返。
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
            PteFlags::EXCLUSIVE,
        ];
        for &flag in &all_flags {
            let pte = PageTableEntry::new(pa, flag);
            assert_eq!(pte.flags(), flag, "标志 {:?} 编解码往返失败", flag);
        }
    }

    /// 验证 RISC-V 各预设标志组合的正确性。
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

    /// RISC-V 的 for_leaf_at_level 不改变标志位。
    #[test]
    fn for_leaf_at_level_is_identity() {
        let flags = PteFlags::kernel_rw();
        assert_eq!(flags.for_leaf_at_level(0), flags);
        assert_eq!(flags.for_leaf_at_level(1), flags);
        assert_eq!(flags.for_leaf_at_level(2), flags);
    }
}

#[cfg(feature = "test-aarch64")]
mod aarch64_specific {
    use super::*;

    /// 每个 AArch64 PteFlags 单独编解码往返。
    #[test]
    fn pte_each_flag_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let all_flags = [
            PteFlags::VALID,
            PteFlags::TABLE,
            PteFlags::MAIR_IDX1,
            PteFlags::AP_UNPRIV,
            PteFlags::AP_RO,
            PteFlags::SH_INNER,
            PteFlags::AF,
            PteFlags::NG,
            PteFlags::PXN,
            PteFlags::UXN,
            PteFlags::EXCLUSIVE,
        ];
        for &flag in &all_flags {
            let pte = PageTableEntry::new(pa, flag);
            assert_eq!(pte.flags(), flag, "标志 {:?} 编解码往返失败", flag);
        }
    }

    /// 验证 AArch64 各预设标志组合的正确性。
    #[test]
    fn pte_flags_presets() {
        let rw = PteFlags::kernel_rw();
        assert!(rw.is_writable());
        assert!(rw.contains(PteFlags::VALID));
        assert!(rw.contains(PteFlags::AF));
        assert!(rw.contains(PteFlags::PXN)); // 数据映射不可执行
        assert!(rw.contains(PteFlags::UXN));
        assert!(!rw.contains(PteFlags::AP_RO)); // 可写 = AP_RO 未设置

        let rx = PteFlags::kernel_rx();
        assert!(!rx.is_writable()); // AP_RO 设置
        assert!(rx.contains(PteFlags::AP_RO));
        assert!(!rx.contains(PteFlags::PXN)); // 内核可执行

        let ro = PteFlags::kernel_ro();
        assert!(!ro.is_writable());
        assert!(ro.contains(PteFlags::PXN)); // 不可执行
        assert!(ro.contains(PteFlags::UXN));
    }

    /// AArch64 的 for_leaf_at_level：Level 0 保留 TABLE 位，Level > 0 清除。
    #[test]
    fn for_leaf_at_level_clears_table_bit() {
        let flags = PteFlags::kernel_rw();
        assert!(flags.contains(PteFlags::TABLE));
        // Level 0（page descriptor）：TABLE 位保留
        assert!(flags.for_leaf_at_level(0).contains(PteFlags::TABLE));
        // Level 1（block descriptor）：TABLE 位清除
        assert!(!flags.for_leaf_at_level(1).contains(PteFlags::TABLE));
        assert!(!flags.for_leaf_at_level(2).contains(PteFlags::TABLE));
    }

    /// AArch64 设备映射使用 MAIR_IDX1。
    #[test]
    fn kernel_device_uses_mair_idx1() {
        let dev = PteFlags::kernel_device();
        assert!(dev.contains(PteFlags::MAIR_IDX1));
        assert!(dev.contains(PteFlags::PXN));
        assert!(dev.contains(PteFlags::UXN));
    }

    /// AArch64 的 is_leaf 依赖层级：Level 0 所有有效项都是叶，Level > 0 看 TABLE 位。
    #[test]
    fn is_leaf_level_dependent() {
        let pa = PhysAddr::new(0x8020_0000);

        // Level 0 page descriptor（有 TABLE 位）→ 叶
        let page_pte = PageTableEntry::new(pa, PteFlags::kernel_rw());
        assert!(page_pte.is_leaf(0));

        // Level 1 block descriptor（无 TABLE 位）→ 叶
        let block_flags = PteFlags::kernel_rw().for_leaf_at_level(1);
        let block_pte = PageTableEntry::new(pa, block_flags);
        assert!(block_pte.is_leaf(1));

        // Level 1 table descriptor（有 TABLE 位）→ 非叶
        let table_pte = PageTableEntry::new_intermediate(pa);
        assert!(!table_pte.is_leaf(1));
    }
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
    assert_eq!(err, PageTableError::MapFailed);
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
    assert_eq!(err, PageTableError::PageNotMapped);
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

/// unmap 唯一叶后，中间节点也被回收——同路径上的其他 VA unmap 应返回 PageNotMapped。
#[test]
fn unmap_reclaims_intermediate_then_sibling_fails() {
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
    assert_eq!(err, PageTableError::PageNotMapped);
}

/// unmap 后中间节点回收：当同表有其他映射时不回收。
#[test]
fn unmap_preserves_intermediate_when_sibling_exists() {
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
#[test]
fn identity_map_range_multi_page() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let start = PhysAddr::new(0x10_0000);
    let end = PhysAddr::new(0x10_3000); // 3 pages

    pt.identity_map_range(start, end, PteFlags::kernel_rw())
        .expect("identity_map_range 应成功");

    for i in 0..3 {
        let va = VirtAddr::new(0x10_0000 + i * config::PAGE_SIZE);
        let (pa, _) = pt.get_mapping(va).expect("应能查到映射");
        assert_eq!(pa, PhysAddr::new(0x10_0000 + i * config::PAGE_SIZE));
    }
}

/// 大页映射：Level 1（2MB）应能映射和查询。
#[test]
fn map_at_level1_huge_page() {
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
#[test]
fn get_mapping_within_huge_page() {
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

/// 大页映射后，不可在同一路径上再映射子页。
#[test]
fn map_page_under_huge_page_fails() {
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
    assert_eq!(err, PageTableError::MapFailed);
}

/// 重复大页映射应返回 MapFailed。
#[test]
fn double_map_at_level_fails() {
    let mut pt = PageTable::create().expect("创建测试页表失败");

    let huge_size = page_size_at_level(1);
    let va = VirtAddr::new(huge_size);
    let pa = PhysAddr::new(0x8020_0000);

    pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
        .expect("首次大页映射应成功");
    let err = pt
        .map_at_level(va, pa, PteFlags::kernel_rw(), 1)
        .expect_err("重复大页映射应失败");
    assert_eq!(err, PageTableError::MapFailed);
}

/// unmap_at_level 应能取消大页映射。
#[test]
fn unmap_at_level1_huge_page() {
    let mut pt = PageTable::create().expect("创建页表");
    let va = VirtAddr::new(0x0000_0000_4000_0000); // 1GB aligned
    let pa = PhysAddr::new(0x0000_0000_4000_0000);
    pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
        .expect("map level1");
    let old_pa = pt.unmap_at_level(va, 1).expect("unmap level1 应成功");
    assert_eq!(old_pa, pa);
    assert!(pt.get_mapping(va).is_none(), "unmap 后应无映射");
}

/// unmap_at_level 目标层级无叶节点时应失败。
#[test]
fn unmap_at_level_wrong_level_fails() {
    let mut pt = PageTable::create().expect("创建页表");
    let va = VirtAddr::new(0x0000_0000_4000_0000);
    let pa = PhysAddr::new(0x0000_0000_4000_0000);
    // 在 level 1 映射，尝试在 level 0 unmap 应失败
    pt.map_at_level(va, pa, PteFlags::kernel_rw(), 1)
        .expect("map level1");
    let err = pt
        .unmap_at_level(va, 0)
        .expect_err("level 0 unmap 大页应失败");
    assert_eq!(err, PageTableError::PageNotMapped);
}
