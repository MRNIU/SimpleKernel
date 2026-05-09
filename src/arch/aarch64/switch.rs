// Copyright The SimpleKernel Contributors

/// AArch64 上下文切换——naked 函数实现
///
/// 使用 `#[unsafe(naked)]` 替代外部 `.S` 文件，消除对 GCC 交叉编译器的依赖。
/// 布局与 `context.rs` 中 `CalleeSavedContext` 严格对应。
use core::arch::naked_asm;

use super::context::CalleeSavedContext;

/// 线程上下文切换
///
/// 保存当前线程的 callee-saved 寄存器到 `prev`，
/// 从 `next` 恢复 callee-saved 寄存器并跳转到保存的 `pc`。
///
/// 寄存器布局（与 `CalleeSavedContext` 一致）：
/// x19-x30[0-95], d8-d15[96-159], sp[160], pc[168]，总计 176 字节。
///
/// # Safety
///
/// - `prev` 必须指向有效的 `CalleeSavedContext`，可写
/// - `next` 必须指向已正确初始化的 `CalleeSavedContext`，可读
/// - 调用者必须确保中断已禁用（避免切换过程中被抢占）
#[unsafe(naked)]
pub unsafe extern "C" fn switch_to(
    _prev: *mut CalleeSavedContext,
    _next: *const CalleeSavedContext,
) {
    naked_asm!(
        // 保存 x19-x30 到 prev (x0)
        "stp x19, x20, [x0, #0]",
        "stp x21, x22, [x0, #16]",
        "stp x23, x24, [x0, #32]",
        "stp x25, x26, [x0, #48]",
        "stp x27, x28, [x0, #64]",
        "stp x29, x30, [x0, #80]",
        // 保存 d8-d15 到 prev (x0)
        "stp d8, d9, [x0, #96]",
        "stp d10, d11, [x0, #112]",
        "stp d12, d13, [x0, #128]",
        "stp d14, d15, [x0, #144]",
        // 保存 sp 和 pc（lr）
        "mov x9, sp",
        "mov x10, x30",
        "stp x9, x10, [x0, #160]",
        // 从 next (x1) 恢复 sp 和 pc
        "ldp x9, x10, [x1, #160]",
        "mov sp, x9",
        // 恢复 d8-d15
        "ldp d8, d9, [x1, #96]",
        "ldp d10, d11, [x1, #112]",
        "ldp d12, d13, [x1, #128]",
        "ldp d14, d15, [x1, #144]",
        // 恢复 x19-x30
        "ldp x19, x20, [x1, #0]",
        "ldp x21, x22, [x1, #16]",
        "ldp x23, x24, [x1, #32]",
        "ldp x25, x26, [x1, #48]",
        "ldp x27, x28, [x1, #64]",
        "ldp x29, x30, [x1, #80]",
        // 跳转到保存的 pc
        "br x10",
    );
}

/// 内核线程汇编入口
///
/// `switch_to` 恢复 callee-saved 上下文后，若 `pc = kernel_thread_entry`，
/// 则跳转到此处。此时：
/// - `x19`（`regs[0]`）= 真正的入口函数 `entry`
/// - `x20`（`regs[1]`）= 参数 `arg`
///
/// 将它们搬到参数寄存器 `x0`/`x1` 后调用 `kernel_thread_bootstrap`。
#[unsafe(naked)]
pub unsafe extern "C" fn kernel_thread_entry() {
    naked_asm!("mov x0, x19", "mov x1, x20", "bl kernel_thread_bootstrap",);
}
