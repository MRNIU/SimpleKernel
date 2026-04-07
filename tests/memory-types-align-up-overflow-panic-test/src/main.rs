//! align_up 在地址空间顶部应 panic 而非静默回绕。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::VirtAddr;

test_harness::test_main!(
    simplekernel::boot::InitLevel::Memory,
    run_test,
    should_panic
);

fn run_test() {
    let near_max = VirtAddr::new(usize::MAX - config::PAGE_SIZE + 2);
    let _ = near_max.align_up();
}
