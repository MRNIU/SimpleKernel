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
    let _frame = frame_allocator::AllocatedFrames::alloc_one()
        .expect("hard IRQ 分配帧应先 panic，不应返回 OOM");
}
