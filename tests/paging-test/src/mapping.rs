//! 仿射类型所有权测试——验证 OwnedPages 的 map/drop/mprotect/unmap 行为。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use frame_allocator::AllocatedFrames;
use paging::{OwnedPages, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_map_basic();
    log::info!("test map_basic ... ok");

    test_map_multi_page();
    log::info!("test map_multi_page ... ok");

    test_map_preexisting();
    log::info!("test map_preexisting ... ok");

    test_map_changes_flags();
    log::info!("test map_changes_flags ... ok");

    test_drop_restores_default_flags();
    log::info!("test drop_restores_default_flags ... ok");

    test_mprotect_changes_flags();
    log::info!("test mprotect_changes_flags ... ok");

    test_unmap_restores_flags_and_returns_frames();
    log::info!("test unmap_restores_flags_and_returns_frames ... ok");

    log::info!("paging-mapping-test: all 7 tests passed");
}

/// 基本映射——VA 从 PA 推导，页表中有记录。
fn test_map_basic() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let mp = OwnedPages::map(frames, PteFlags::kernel_rw());

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
    let _mp = OwnedPages::map(frames, PteFlags::kernel_rw());

    let guard = paging::kernel_page_table().lock();
    for i in 0..3 {
        let pa = pa_start + i * config::PAGE_SIZE;
        assert!(guard.get_mapping(pa.to_virt()).is_some());
    }
}

/// 对已有背景映射的帧调用 map 应成功（幂等）。
fn test_map_preexisting() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let va = pa.to_virt();

    // 背景映射已存在（kernel_rw），map 以相同 flags 应幂等成功
    let mp = OwnedPages::map(frames, PteFlags::kernel_rw());
    assert_eq!(mp.vaddr(), va);

    let guard = paging::kernel_page_table().lock();
    let (got_pa, _) = guard.get_mapping(va).expect("映射应存在");
    assert_eq!(got_pa, pa);
}

/// map 可以覆盖背景映射的权限（从 kernel_rw 改为 kernel_ro）。
fn test_map_changes_flags() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();

    // 背景映射为 kernel_rw，map 以 kernel_ro 应更新 PTE flags
    let mp = OwnedPages::map(frames, PteFlags::kernel_ro());

    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("映射应存在");
    assert!(!flags.is_writable(), "map(kernel_ro) 应设为只读");
    drop(guard);
    drop(mp);
}

/// Drop 恢复 PTE 为默认 kernel_rw（不删除映射）。
fn test_drop_restores_default_flags() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    // 使用 kernel_ro 映射，drop 后应恢复为 kernel_rw
    let mp = OwnedPages::map(frames, PteFlags::kernel_ro());

    {
        let guard = paging::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(!flags.is_writable());
    }
    drop(mp);
    // PTE 仍存在，但权限已恢复为 kernel_rw
    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("drop 后映射仍应存在");
    assert!(flags.is_writable());
}

/// mprotect 修改权限。
fn test_mprotect_changes_flags() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mut mp = OwnedPages::map(frames, PteFlags::kernel_rw());

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

/// unmap 恢复默认权限并返回 UnmappedFrames。
fn test_unmap_restores_flags_and_returns_frames() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let va = pa.to_virt();
    // 使用 kernel_ro 映射，unmap 后应恢复为 kernel_rw
    let mp = OwnedPages::map(frames, PteFlags::kernel_ro());

    let unmapped = mp.unmap();
    assert_eq!(unmapped.start_paddr(), pa);

    // PTE 仍存在，但权限已恢复为 kernel_rw
    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("unmap 后映射仍应存在");
    assert!(flags.is_writable());
}
