// Copyright The SimpleKernel Contributors

//! hard IRQ 中禁止分配物理帧。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test, should_panic);

/// hard IRQ 上下文中调用 `AllocatedFrames::alloc_one()` 必须 fail-fast。
fn run_test() {
    let _irq = interrupt_state::HardIrqGuard::enter();
    let _frame = frame_allocator::AllocatedFrames::alloc_one().unwrap_or_else(|error| {
        panic!(
            "frame-test/alloc-in-hardirq-panic: hard IRQ 分配帧未先触发上下文 panic，而是返回错误: requested_pages=1, error={error:?}"
        )
    });
}
