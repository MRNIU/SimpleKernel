// Copyright The SimpleKernel Contributors

//! should_panic 测试：同一核心递归加锁应触发 panic。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(
    simplekernel::boot::InitLevel::Full,
    test_recursive_lock_panics,
    should_panic
);

/// 同一核心递归加锁应触发 panic。
fn test_recursive_lock_panics() {
    let lock = sync::SpinLock::new(0u32, "recursive", sync::lock_level::UNSPECIFIED);
    let _guard1 = lock.lock();
    // 同一核心再次 lock → check_recursive 检测到 → panic
    let _guard2 = lock.lock();
}
