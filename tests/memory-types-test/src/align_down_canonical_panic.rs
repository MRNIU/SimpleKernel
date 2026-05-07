//! align_down_to 不应生成 canonical hole 中的虚拟地址。

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

/// 大粒度向下对齐高半区边界地址应触发 canonical 校验 panic。
fn run_test() {
    let high_half_base = VirtAddr::new(usize::MAX << (arch::VA_BITS - 1));
    let _ = high_half_base.align_down_to(1usize << arch::VA_BITS);
}
