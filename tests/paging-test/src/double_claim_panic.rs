//! 双重 claim 应 panic——验证 CLAIMED 位检测双重所有权。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::mem::ManuallyDrop;

use frame_allocator::AllocatedFrames;
use paging::{MappedPages, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test, should_panic);

/// 用 unsafe ptr::read 复制同一 AllocatedFrames，
/// 触发 double claim → 应 panic。
fn run_test() {
    let frames = ManuallyDrop::new(AllocatedFrames::alloc(1).expect("alloc frames"));

    // SAFETY: 故意通过 ptr::read 复制帧以触发 CLAIMED panic（仅用于测试）。
    // 两个副本均不会触发 Drop——_mp1/mp2 的 release/drop 负责清理。
    let frames1: AllocatedFrames = unsafe { core::ptr::read(&*frames) };
    let frames2: AllocatedFrames = unsafe { core::ptr::read(&*frames) };

    let _mp1 = MappedPages::claim(frames1, PteFlags::kernel_rw());

    // 与 _mp1 覆盖同一物理帧——CLAIMED 已设置，应 panic
    let _mp2 = MappedPages::claim(frames2, PteFlags::kernel_rw());
}
