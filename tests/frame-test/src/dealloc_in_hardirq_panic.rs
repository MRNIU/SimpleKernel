// Copyright The SimpleKernel Contributors

//! hard IRQ 中禁止释放物理帧。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test, should_panic);

/// hard IRQ 上下文中 drop `AllocatedFrames` 必须 fail-fast。
fn run_test() {
    let frame = frame_allocator::AllocatedFrames::alloc_one().unwrap_or_else(|error| {
        panic!(
            "frame-test/dealloc-in-hardirq-panic: 预先分配物理帧失败: requested_pages=1, error={error:?}"
        )
    });

    let _irq = interrupt_state::HardIrqGuard::enter();
    drop(frame);
}
