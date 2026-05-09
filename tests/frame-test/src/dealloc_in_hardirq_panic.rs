// Copyright The SimpleKernel Contributors

//! hard IRQ 中禁止释放物理帧。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test, should_panic);

/// hard IRQ 上下文中 drop `AllocatedFrames` 必须 fail-fast。
fn run_test() {
    let frame = frame_allocator::AllocatedFrames::alloc_one().expect("预先分配一页物理帧");

    let _irq = interrupt_state::HardIrqGuard::enter();
    drop(frame);
}
