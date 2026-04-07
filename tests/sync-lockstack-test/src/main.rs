//! LockStack 测试——验证锁序检查、push/pop 往返。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_empty_stack_allows_any_level();
    log::info!("test empty_stack_allows_any_level ... ok");

    test_unspecified_skips_order_check();
    log::info!("test unspecified_skips_order_check ... ok");

    test_unspecified_on_top_skips_check();
    log::info!("test unspecified_on_top_skips_check ... ok");

    test_normal_levels_must_increase();
    log::info!("test normal_levels_must_increase ... ok");

    test_push_pop_roundtrip();
    log::info!("test push_pop_roundtrip ... ok");

    log::info!("sync-lockstack-test: all 5 tests passed");
}

/// 空栈允许任意级别——SCHED、CONSOLE、UNSPECIFIED 均通过。
fn test_empty_stack_allows_any_level() {
    let stack = sync::lock_stack::LockStack::new();
    assert!(stack.check_order(sync::lock_level::SCHED));
    assert!(stack.check_order(sync::lock_level::CONSOLE));
    assert!(stack.check_order(sync::lock_level::UNSPECIFIED));
}

/// 栈顶为普通级别时，UNSPECIFIED 跳过顺序检查。
fn test_unspecified_skips_order_check() {
    let mut stack = sync::lock_stack::LockStack::new();
    stack.push(core::ptr::null(), sync::lock_level::CONSOLE);
    assert!(stack.check_order(sync::lock_level::UNSPECIFIED));
}

/// 栈顶为 UNSPECIFIED 时，任何普通级别也跳过检查。
fn test_unspecified_on_top_skips_check() {
    let mut stack = sync::lock_stack::LockStack::new();
    stack.push(core::ptr::null(), sync::lock_level::UNSPECIFIED);
    assert!(stack.check_order(sync::lock_level::SCHED));
}

/// 普通级别必须严格递增——SCHED → TASK_TABLE 通过，SCHED → SCHED 失败。
fn test_normal_levels_must_increase() {
    let mut stack = sync::lock_stack::LockStack::new();
    stack.push(core::ptr::null(), sync::lock_level::SCHED);
    assert!(stack.check_order(sync::lock_level::TASK_TABLE));
    assert!(!stack.check_order(sync::lock_level::SCHED));
}

/// push/pop 往返——push 后 depth=1，pop 后 depth=0。
fn test_push_pop_roundtrip() {
    let mut stack = sync::lock_stack::LockStack::new();
    let ptr = core::ptr::null();
    stack.push(ptr, sync::lock_level::SCHED);
    assert_eq!(stack.depth(), 1);
    stack.pop(ptr);
    assert_eq!(stack.depth(), 0);
}
