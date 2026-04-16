//! 仿射类型映射测试——验证 MappedPages 的 map/drop/mprotect/unmap 行为。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use frame_allocator::AllocatedFrames;
use paging::{MappedPages, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_map_basic();
    log::info!("test map_basic ... ok");

    test_map_multi_page();
    log::info!("test map_multi_page ... ok");

    test_drop_unmaps();
    log::info!("test drop_unmaps ... ok");

    test_mprotect_changes_flags();
    log::info!("test mprotect_changes_flags ... ok");

    test_unmap_returns_unmapped_frames();
    log::info!("test unmap_returns_unmapped_frames ... ok");

    log::info!("paging-mapping-test: all 5 tests passed");
}

/// 基本映射——VA 从 PA 推导，页表中有记录。
fn test_map_basic() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    assert_eq!(mp.vaddr(), pa.to_virt());
    assert_eq!(mp.size(), config::PAGE_SIZE);

    let guard = paging::kernel_page_table().lock();
    let (got_pa, _) = guard.get_mapping(pa.to_virt()).expect("映射应存在");
    assert_eq!(got_pa, pa);
}

/// 多页映射。
fn test_map_multi_page() {
    let frames = AllocatedFrames::alloc(3).expect("alloc frames");
    let pa_start = frames.start_paddr();
    let _mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    let guard = paging::kernel_page_table().lock();
    for i in 0..3 {
        let pa = pa_start + i * config::PAGE_SIZE;
        assert!(guard.get_mapping(pa.to_virt()).is_some());
    }
}

/// Drop 自动 unmap。
fn test_drop_unmaps() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    {
        let guard = paging::kernel_page_table().lock();
        assert!(guard.get_mapping(va).is_some());
    }
    drop(mp);
    let guard = paging::kernel_page_table().lock();
    assert!(guard.get_mapping(va).is_none());
}

/// mprotect 修改权限。
fn test_mprotect_changes_flags() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mut mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    {
        let guard = paging::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(flags.is_writable());
    }

    mp.mprotect(PteFlags::kernel_ro());

    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("映射应存在");
    assert!(!flags.is_writable());
}

/// unmap 返回 UnmappedFrames 并清除页表。
fn test_unmap_returns_unmapped_frames() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let va = pa.to_virt();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    let unmapped = mp.unmap();
    assert_eq!(unmapped.start_paddr(), pa);

    let guard = paging::kernel_page_table().lock();
    assert!(guard.get_mapping(va).is_none());
}
