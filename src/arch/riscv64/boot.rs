// Copyright The SimpleKernel Contributors

use core::arch::global_asm;

// KERNEL_STACK_SIZE 必须是 2 的幂，方便用移位替代乘法
// （global_asm! 走 LLVM 汇编器，不自动继承 -march=rv64gc 的 M 扩展）
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
.extern __global_pointer$
.extern BOOT_STACK

_boot:
    // 初始化 gp 寄存器。每个 hart 都必须满足 RISC-V psABI 的全局指针契约，
    // 不能依赖 a1/opaque 是否携带 DTB 地址。
    // @see riscv-abi.pdf#9.1.4
.option push
.option norelax
1:  auipc gp, %pcrel_hi(__global_pointer$)
    addi  gp, gp, %pcrel_lo(1b)
.option pop

    // 当前平台契约要求 hart id 为 0..MAX_CORE_COUNT 的 dense 编号。
    li t1, {MAX_CORE_COUNT}
    bgeu a0, t1, .Lboot_stack_overflow
    add t0, a0, 1
    slli t0, t0, {KERNEL_STACK_SIZE_LOG2}
    la sp, BOOT_STACK
    add sp, sp, t0

    // 将 hart id 写入 tp
    mv tp, a0

    // 保存 SBI 传递的参数
    addi sp, sp, -8*2
    sd a0, (0 * 8)(sp)     // a0: 启动核 id
    sd a1, (1 * 8)(sp)     // a1: dtb 地址

    call _start
    wfi

.Lboot_stack_overflow:
    wfi
    j .Lboot_stack_overflow
"#,
    KERNEL_STACK_SIZE_LOG2 = const KERNEL_STACK_SIZE_LOG2,
    MAX_CORE_COUNT = const config::MAX_CORE_COUNT,
);
