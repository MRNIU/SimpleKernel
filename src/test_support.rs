// Copyright The SimpleKernel Contributors

//! 独立系统测试专用钩子。
//!
//! 本模块只在 `test-support` feature 下编译，生产内核不暴露这些入口。

/// 注册一个 RISC-V store page fault 预期恢复点。
///
/// # Panics
///
/// 如果目标架构不是 RISC-V，本函数不可用。
#[cfg(target_arch = "riscv64")]
pub fn expect_riscv64_store_page_fault(
    target_core_id: usize,
    fault_addr: usize,
    fault_pc: usize,
    resume_pc: usize,
) {
    crate::arch::riscv64::interrupt::expect_store_page_fault_for_test(
        target_core_id,
        fault_addr,
        fault_pc,
        resume_pc,
    );
}

/// 返回预期 RISC-V store page fault 是否已经被目标核心命中。
#[cfg(target_arch = "riscv64")]
pub fn riscv64_store_page_fault_observed() -> bool {
    crate::arch::riscv64::interrupt::store_page_fault_observed_for_test()
}
