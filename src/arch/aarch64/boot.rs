// Copyright The SimpleKernel Contributors

use core::arch::global_asm;

const _: () = assert!(
    config::KERNEL_STACK_SIZE.is_power_of_two(),
    "KERNEL_STACK_SIZE must be a power of two"
);
const KERNEL_STACK_SIZE_LOG2: u32 = config::KERNEL_STACK_SIZE.trailing_zeros();

global_asm!(
    r#"
.section .text.boot
.global _boot
.type _boot, @function
.extern _start
.extern BOOT_STACK

_boot:
    // 当前平台契约要求 MPIDR Aff0 为 0..MAX_CORE_COUNT 的 dense 编号。
    mrs x10, mpidr_el1
    and x10, x10, #0xFF
    cmp x10, #{MAX_CORE_COUNT}
    b.hs 2f
    add x10, x10, #1
    lsl x10, x10, #{KERNEL_STACK_SIZE_LOG2}
    adrp x11, BOOT_STACK
    add x11, x11, :lo12:BOOT_STACK
    add x11, x11, x10
    mov sp, x11

    // 初始化 TPIDR_EL1 为 0（percpu_init 会设置正确的 per-CPU 基地址）
    msr tpidr_el1, xzr

    // 启用 EL0/EL1 的 FP/SIMD 访问，避免 hardfloat 目标生成的浮点指令触发异常
    mrs x9, cpacr_el1
    movz x11, #0x30, lsl #16
    orr x9, x9, x11
    msr cpacr_el1, x9
    isb

    // 保存传递的参数
    stp x0, x1, [sp, #-16]!

    bl _start
    b .

2:
    wfi
    b 2b
"#,
    KERNEL_STACK_SIZE_LOG2 = const KERNEL_STACK_SIZE_LOG2,
    MAX_CORE_COUNT = const config::MAX_CORE_COUNT,
);
