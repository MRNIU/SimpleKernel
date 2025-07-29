/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief boot S
 */

// clang-format off

// 声明所属段
.section .text.boot
.global _boot
.type _boot, @function
.extern _start
_boot:
    // 关中断
    cli
    // 设置栈指针
    mov $stack_top, %rsp
    call _start
    hlt
    ret

// 声明所属段
.section .bss.boot
// 16 字节对齐
.align 16
.global stack_top
// 栈
stack_top:
    // 跳过 16KB
    .space 4096 * 4

// clang-format on
