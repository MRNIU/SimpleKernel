// Copyright The SimpleKernel Contributors

//! PhysAddr 加法结果超出 PA_BITS 范围应 panic。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use memory_types::PhysAddr;

test_harness::test_main!(
    simplekernel::boot::InitLevel::Memory,
    run_test,
    should_panic
);

fn run_test() {
    let near_max = PhysAddr::new((1usize << arch_primitives::PA_BITS) - 2);
    let _ = near_max + 4;
}
