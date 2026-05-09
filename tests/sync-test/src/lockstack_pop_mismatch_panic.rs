// Copyright The SimpleKernel Contributors

//! should_panic 测试：pop 时栈顶指针不匹配应 panic。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(
    simplekernel::boot::InitLevel::Full,
    test_pop_mismatch_panics,
    should_panic
);

/// pop 时栈顶指针不匹配应 panic。
fn test_pop_mismatch_panics() {
    let mut stack = sync::lock_stack::LockStack::new();
    let ptr_a = 0x1000 as *const ();
    let ptr_b = 0x2000 as *const ();
    stack.push(ptr_a, sync::lock_level::SCHED);
    stack.pop(ptr_b);
}
