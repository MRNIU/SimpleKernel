//! VirtAddr 加法结果不满足规范化应 panic。

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
    let near_boundary = VirtAddr::new((1usize << (arch::VA_BITS - 1)) - 2);
    let _ = near_boundary + 4;
}
