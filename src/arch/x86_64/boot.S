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

// 以下是多核启动入口
.section .text
.global ap_start16
.global ap_start16_end
.code16
ap_start16:
    cli

    // 清空 tlb
    xor %eax, %eax
    mov %eax, %cr3

    // 设置数据段寄存器
    mov %cs, %ax
    mov %ax, %ds

    // 计算 gdt 地址并加载
    mov $gdtdesc, %ebx
    sub $ap_start16, %ebx
    data32 lgdt (%ebx)

    // 启用保护模式
    mov %cr0, %eax
    // cr0 的第一位为保护模式位
    or $0x1,%eax
    mov %eax, %cr0

    // 跳转到保护模式
    jmpl    $8, $(ap_start)

// 临时使用的 GDT 描述符
.align 8
gdt:
    .word 0, 0
    .byte 0, 0, 0, 0
    .word 0xFFFF, 0
    .byte 0, 0x9A, 0xCF, 0
gdtdesc:
    .word (gdtdesc - gdt - 1)
    .long gdt

ap_start16_end:

.code32
.align 16
.globl ap_start
ap_start:
    movl $0x233, %eax
    hlt


/* This matches struct sipi_param */
.align  4
.globl  sipi_params
sipi_params:
idt_ptr:
    .long 0
sipi_stack_top:
    .long 0
sipi_stack_size:
    .long 0
msr_table_ptr:
    .long 0
msr_count:
    .long 0
c_handler:
    .long 0
ap_count:
    .long 0


// clang-format on
