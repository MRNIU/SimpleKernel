// Copyright The SimpleKernel Contributors

//! 帧分配器测试——验证 RAII 帧分配与回收。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use frame_allocator::AllocatedFrames;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_alloc_one_frame();
    log::info!("test alloc_one_frame ... ok");

    test_alloc_multiple_frames();
    log::info!("test alloc_multiple_frames ... ok");

    test_alloc_dealloc_realloc();
    log::info!("test alloc_dealloc_realloc ... ok");

    log::info!("frame-alloc-test: all 3 tests passed");
}

/// 分配单帧后帧计数应为 1，地址应页对齐。
fn test_alloc_one_frame() {
    let frame = AllocatedFrames::alloc_one().expect("alloc_one 应成功");
    assert_eq!(frame.page_count(), 1);
    assert!(frame.start_paddr().is_aligned());
}

/// 分配多帧后帧计数应正确。
fn test_alloc_multiple_frames() {
    let frames = AllocatedFrames::alloc(4).expect("alloc(4) 应成功");
    assert_eq!(frames.page_count(), 4);
}

/// 帧 drop 后应能重新分配。
fn test_alloc_dealloc_realloc() {
    {
        let _frame = AllocatedFrames::alloc_one().expect("分配");
    }
    let frame2 = AllocatedFrames::alloc_one().expect("重新分配应成功");
    assert!(frame2.start_paddr().is_aligned());
}
