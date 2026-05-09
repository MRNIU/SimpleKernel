// Copyright The SimpleKernel Contributors

//! TLB shootdown 超时诊断测试。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

use simplekernel::boot::InitLevel;

test_harness::test_main!(InitLevel::Interrupt, run_test, should_panic);

/// 等待远端 ack 超时必须 fail-fast，而不是永久自旋。
fn run_test() {
    simplekernel::tlb_shootdown::trigger_ack_timeout_for_test();
}
