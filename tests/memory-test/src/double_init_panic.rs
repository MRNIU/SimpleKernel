//! memory::init 二次调用应 fail-fast。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(
    simplekernel::boot::InitLevel::Memory,
    run_test,
    should_panic
);

/// `kernel_init(InitLevel::Memory)` 已初始化后，再调用 `memory::init()` 应 panic。
fn run_test() {
    memory::init();
}
