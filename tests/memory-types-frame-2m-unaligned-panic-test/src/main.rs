//! Frame<Page2M>::new 对非对齐的 4K 页号应 panic。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::{Frame, Page2M};

test_harness::test_main!(
    simplekernel::boot::InitLevel::Memory,
    run_test,
    should_panic
);

fn run_test() {
    let _ = Frame::<Page2M>::new(1);
}
