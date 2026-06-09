// Copyright The SimpleKernel Contributors

//! Frame::new 应拒绝无法转换为有效物理地址的页号。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::Frame;

test_harness::test_main!(
    simplekernel::boot::InitLevel::Memory,
    run_test,
    should_panic
);

/// 超过物理地址位宽的页号不能构造为 `Frame`。
fn run_test() {
    let frame_count_limit = 1usize << (arch_primitives::PA_BITS - config::PAGE_SIZE_BITS);
    let _ = Frame::new(frame_count_limit);
}
