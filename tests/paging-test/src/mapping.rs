//! 仿射类型所有权测试——验证 MappedPages 的 claim/release/mprotect 行为。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use frame_allocator::AllocatedFrames;
use paging::{MappedPages, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_claim_basic();
    log::info!("test claim_basic ... ok");

    test_claim_multi_page();
    log::info!("test claim_multi_page ... ok");

    test_release_clears_claimed();
    log::info!("test release_clears_claimed ... ok");

    test_drop_writes_poison();
    log::info!("test drop_writes_poison ... ok");

    test_mprotect_preserves_claimed();
    log::info!("test mprotect_preserves_claimed ... ok");

    test_release_returns_unmapped_frames();
    log::info!("test release_returns_unmapped_frames ... ok");

    test_reclaim_after_release();
    log::info!("test reclaim_after_release ... ok");

    log::info!("paging-mapping-test: all 7 tests passed");
}

/// claim 基本功能——CLAIMED 位被设置，权限正确。
fn test_claim_basic() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    assert_eq!(mp.vaddr(), pa.to_virt());
    assert_eq!(mp.size(), config::PAGE_SIZE);

    // PTE 应包含 CLAIMED 位
    let guard = paging::kernel_page_table().lock();
    let (got_pa, flags) = guard.get_mapping(pa.to_virt()).expect("映射应存在");
    assert_eq!(got_pa, pa);
    assert!(flags.is_claimed());
    assert!(flags.is_writable());
}

/// 多页 claim。
fn test_claim_multi_page() {
    let frames = AllocatedFrames::alloc(3).expect("alloc frames");
    let pa_start = frames.start_paddr();
    let _mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    let guard = paging::kernel_page_table().lock();
    for i in 0..3 {
        let pa = pa_start + i * config::PAGE_SIZE;
        let (_, flags) = guard.get_mapping(pa.to_virt()).expect("映射应存在");
        assert!(flags.is_claimed());
    }
}

/// release 清除 CLAIMED 位并恢复默认权限。
fn test_release_clears_claimed() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mp = MappedPages::claim(frames, PteFlags::kernel_ro());

    // claim 后 CLAIMED=1，权限=RO
    {
        let guard = paging::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(flags.is_claimed());
        assert!(!flags.is_writable());
    }

    let _unmapped = mp.release();

    // release 后 CLAIMED=0，权限恢复为 kernel_rw，PTE 仍存在（SAS 不删 PTE）
    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("PTE 应仍存在");
    assert!(!flags.is_claimed());
    assert!(flags.is_writable());
}

/// drop 后页面内容被填充 poison pattern。
fn test_drop_writes_poison() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    // 写入已知数据
    // SAFETY: 页已 claimed 且可写
    unsafe {
        core::ptr::write_bytes(va.as_mut_ptr::<u8>(), 0x42, config::PAGE_SIZE);
    }

    drop(mp);

    // drop 后内容应为 poison pattern
    // SAFETY: SAS 下 PTE 仍有效（CLAIMED 已清除），读取是安全的
    let first_byte = unsafe { *(va.as_usize() as *const u8) };
    assert_eq!(
        first_byte,
        config::FREED_PAGE_POISON,
        "drop 后页面未被 poison 填充"
    );
}

/// mprotect 保留 CLAIMED 位。
fn test_mprotect_preserves_claimed() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mut mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    mp.mprotect(PteFlags::kernel_ro());

    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("映射应存在");
    assert!(flags.is_claimed(), "mprotect 不应清除 CLAIMED 位");
    assert!(!flags.is_writable());
}

/// release 返回 UnmappedFrames。
fn test_release_returns_unmapped_frames() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    let unmapped = mp.release();
    assert_eq!(unmapped.start_paddr(), pa);
}

/// release 后同一帧可被再次 claim。
fn test_reclaim_after_release() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());
    let _unmapped = mp.release();
    // _unmapped 的 Drop 将帧归还 buddy

    // 再次分配（buddy 可能返回同一帧）+ claim 应成功
    let frames2 = AllocatedFrames::alloc(1).expect("alloc frames 2");
    let _mp2 = MappedPages::claim(frames2, PteFlags::kernel_rw());
    // 如果 CLAIMED 位未正确清除，此处会 panic
}
