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
    let frame = AllocatedFrames::alloc_one().unwrap_or_else(|error| {
        panic!("frame-test/alloc: alloc_one 失败: requested_pages=1, error={error:?}")
    });
    assert_eq!(frame.page_count(), 1, "单帧分配页数错误");
    assert!(
        frame.start_paddr().is_aligned(),
        "单帧分配返回未页对齐地址: start={}",
        frame.start_paddr()
    );
}

/// 分配多帧后帧计数应正确。
fn test_alloc_multiple_frames() {
    let frames = AllocatedFrames::alloc(4).unwrap_or_else(|error| {
        panic!("frame-test/alloc: alloc(4) 失败: requested_pages=4, error={error:?}")
    });
    assert_eq!(frames.page_count(), 4, "多帧分配页数错误");
}

/// 帧 drop 后应能重新分配。
fn test_alloc_dealloc_realloc() {
    {
        let _frame = AllocatedFrames::alloc_one().unwrap_or_else(|error| {
            panic!("frame-test/alloc: drop 前 alloc_one 失败: requested_pages=1, error={error:?}")
        });
    }
    let frame2 = AllocatedFrames::alloc_one().unwrap_or_else(|error| {
        panic!("frame-test/alloc: drop 后重新 alloc_one 失败: requested_pages=1, error={error:?}")
    });
    assert!(
        frame2.start_paddr().is_aligned(),
        "重新分配返回未页对齐地址: start={}",
        frame2.start_paddr()
    );
}
