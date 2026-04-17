//! 仿射类型所有权测试——验证 OwnedPages 的 new/drop/set_flags 行为。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use frame_allocator::AllocatedFrames;
use paging::{OwnedPages, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_new_basic();
    log::info!("test new_basic ... ok");

    test_new_multi_page();
    log::info!("test new_multi_page ... ok");

    test_new_preexisting();
    log::info!("test new_preexisting ... ok");

    test_new_changes_flags();
    log::info!("test new_changes_flags ... ok");

    test_drop_restores_default_flags();
    log::info!("test drop_restores_default_flags ... ok");

    test_set_flags_changes_flags();
    log::info!("test set_flags_changes_flags ... ok");

    log::info!("paging-mapping-test: all 6 tests passed");
}

/// 基本构造——VA 从 PA 推导，页表中有记录。
fn test_new_basic() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let mp = OwnedPages::new(frames, PteFlags::kernel_rw());

    assert_eq!(mp.vaddr(), pa.to_virt());
    assert_eq!(mp.size(), config::PAGE_SIZE);

    let guard = paging::kernel_page_table().lock();
    let (got_pa, _flags) = guard.get_mapping(pa.to_virt()).expect("页表项应存在");
    assert_eq!(got_pa, pa);
}

/// 多页构造。
fn test_new_multi_page() {
    let frames = AllocatedFrames::alloc(3).expect("alloc frames");
    let pa_start = frames.start_paddr();
    let _mp = OwnedPages::new(frames, PteFlags::kernel_rw());

    let guard = paging::kernel_page_table().lock();
    for i in 0..3 {
        let pa = pa_start + i * config::PAGE_SIZE;
        assert!(guard.get_mapping(pa.to_virt()).is_some());
    }
}

/// 对已有背景权限的帧调用 new 应成功（幂等）。
fn test_new_preexisting() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let va = pa.to_virt();

    // 背景权限已存在（kernel_rw），new 以相同 flags 应幂等成功
    let mp = OwnedPages::new(frames, PteFlags::kernel_rw());
    assert_eq!(mp.vaddr(), va);

    let guard = paging::kernel_page_table().lock();
    let (got_pa, _) = guard.get_mapping(va).expect("页表项应存在");
    assert_eq!(got_pa, pa);
}

/// new 可以覆盖背景层权限（从 kernel_rw 改为 kernel_ro）。
fn test_new_changes_flags() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();

    // 背景层为 kernel_rw，new 以 kernel_ro 应更新 PTE flags
    let mp = OwnedPages::new(frames, PteFlags::kernel_ro());

    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("页表项应存在");
    assert!(!flags.is_writable(), "new(kernel_ro) 应设为只读");
    drop(guard);
    drop(mp);
}

/// Drop 恢复 PTE 为默认 kernel_rw（不删除页表项）。
fn test_drop_restores_default_flags() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    // 使用 kernel_ro 覆盖，drop 后应恢复为 kernel_rw
    let mp = OwnedPages::new(frames, PteFlags::kernel_ro());

    {
        let guard = paging::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("页表项应存在");
        assert!(!flags.is_writable());
    }
    drop(mp);
    // PTE 仍存在，但权限已恢复为 kernel_rw
    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("drop 后页表项仍应存在");
    assert!(flags.is_writable());
}

/// set_flags 修改权限。
fn test_set_flags_changes_flags() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mut mp = OwnedPages::new(frames, PteFlags::kernel_rw());

    {
        let guard = paging::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("页表项应存在");
        assert!(flags.is_writable());
    }

    mp.set_flags(PteFlags::kernel_ro());

    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("页表项应存在");
    assert!(!flags.is_writable());
}
