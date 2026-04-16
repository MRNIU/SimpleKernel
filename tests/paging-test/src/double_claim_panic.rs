//! should_panic 测试——对已被 OwnedPages 声明的帧再次 new 应 panic。
//!
//! 通过手动设置 PTE 的 CLAIMED 位模拟另一个 OwnedPages 已持有该帧，
//! 验证 `OwnedPages::new` 能检测双重所有权并 panic。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::mem::ManuallyDrop;

use frame_allocator::AllocatedFrames;
use paging::{OwnedPages, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test, should_panic);

/// 手动设置 CLAIMED 位后 OwnedPages::new 应 panic。
fn run_test() {
    let frames = AllocatedFrames::alloc(1).expect("分配帧");
    let va = frames.start_paddr().to_virt();

    // 手动在 PTE 上设置 CLAIMED 位——模拟另一个 OwnedPages 已持有该帧
    {
        let mut guard = paging::kernel_page_table().lock();
        guard
            .update_flags(va, PteFlags::kernel_rw().with_claimed(true))
            .expect("设置 CLAIMED");
    }

    // 尝试创建 OwnedPages——应 panic：CLAIMED 已设置
    let _owned = ManuallyDrop::new(OwnedPages::new(frames, PteFlags::kernel_rw()));
}
