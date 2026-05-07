//! frame_allocator::init 应拒绝与 free 范围重叠的 reserved 描述。

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

/// `reserved` 与 free 范围重叠时，初始化必须在入 buddy 前 panic。
fn run_test() {
    let free_start = PhysAddr::new(0x8040_0000);
    let reserved_start = free_start + config::PAGE_SIZE;

    // SAFETY: 测试故意传入与 free 范围重叠的 reserved 描述，验证 init 会 fail-fast。
    unsafe {
        frame_allocator::init(free_start, config::PAGE_SIZE * 4, &[(reserved_start, 1)]);
    }
}
