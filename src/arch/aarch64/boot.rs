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

_boot:
    // 获取启动核 ID
    mrs x10, mpidr_el1
    and x10, x10, #0xFF

    // 按照每个 core 设置栈地址：(core_id + 1) << log2(KERNEL_STACK_SIZE)
    add x10, x10, #1
    lsl x10, x10, #{KERNEL_STACK_SIZE_LOG2}
    adrp x11, stack_top
    add x11, x11, :lo12:stack_top
    add x11, x11, x10
    mov sp, x11

    // 初始化 TPIDR_EL1 为 0（percpu_init 会设置正确的 per-CPU 基地址）
    msr tpidr_el1, xzr

    // 保存传递的参数
    stp x0, x1, [sp, #-16]!

    bl _start
    b .

.section .bss.boot
.align 16
.global stack_top
stack_top:
    .space {KERNEL_STACK_SIZE} * {MAX_CORE_COUNT}
"#,
    KERNEL_STACK_SIZE_LOG2 = const KERNEL_STACK_SIZE_LOG2,
    KERNEL_STACK_SIZE = const config::KERNEL_STACK_SIZE,
    MAX_CORE_COUNT = const config::MAX_CORE_COUNT,
);
