// Copyright The SimpleKernel Contributors

/// RISC-V 64 上下文切换——naked 函数实现
///
/// 使用 `#[unsafe(naked)]` 替代外部 `.S` 文件，消除对 GCC 交叉编译器的依赖。
/// 布局与 `context.rs` 中 `CalleeSavedContext` 严格对应。
use core::arch::naked_asm;

use super::context::CalleeSavedContext;

/// 线程上下文切换
///
/// 保存当前线程的 callee-saved 寄存器到 `prev`，
/// 从 `next` 恢复 callee-saved 寄存器，通过 `ret` 跳转到 `next.ra`。
///
/// 寄存器布局（与 `CalleeSavedContext` 一致）：
/// ra[0], sp[1], s0[2]..s11[13]，每个 8 字节。
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
        // 保存 callee-saved 寄存器到 prev (a0)
        "sd ra,   0*8(a0)",
        "sd sp,   1*8(a0)",
        "sd s0,   2*8(a0)",
        "sd s1,   3*8(a0)",
        "sd s2,   4*8(a0)",
        "sd s3,   5*8(a0)",
        "sd s4,   6*8(a0)",
        "sd s5,   7*8(a0)",
        "sd s6,   8*8(a0)",
        "sd s7,   9*8(a0)",
        "sd s8,  10*8(a0)",
        "sd s9,  11*8(a0)",
        "sd s10, 12*8(a0)",
        "sd s11, 13*8(a0)",
        // 从 next (a1) 恢复 callee-saved 寄存器
        "ld ra,   0*8(a1)",
        "ld sp,   1*8(a1)",
        "ld s0,   2*8(a1)",
        "ld s1,   3*8(a1)",
        "ld s2,   4*8(a1)",
        "ld s3,   5*8(a1)",
        "ld s4,   6*8(a1)",
        "ld s5,   7*8(a1)",
        "ld s6,   8*8(a1)",
        "ld s7,   9*8(a1)",
        "ld s8,  10*8(a1)",
        "ld s9,  11*8(a1)",
        "ld s10, 12*8(a1)",
        "ld s11, 13*8(a1)",
        "ret",
    );
}

/// 内核线程汇编入口
///
/// `switch_to` 恢复 callee-saved 上下文后，若 `ra = kernel_thread_entry`，
/// 则跳转到此处。此时：
/// - `s0` = 真正的入口函数 `entry`
/// - `s1` = 参数 `arg`
///
/// 将它们搬到参数寄存器 `a0`/`a1` 后调用 `kernel_thread_bootstrap`。
#[unsafe(naked)]
pub unsafe extern "C" fn kernel_thread_entry() {
    naked_asm!("mv a0, s0", "mv a1, s1", "call kernel_thread_bootstrap",);
}
