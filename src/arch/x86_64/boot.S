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
.text
.global ap_start16
.global ap_start16_end
.code16
ap_start16:
	cli

    // 清空 tlb
    xorl	%eax, %eax
	movl	%eax, %cr3

	// 设置数据段寄存器
	movw	%cs, %ax
	movw	%ax, %ds

    // 计算 gdt 地址并加载
	movl	$gdtaddr, %ebx
	subl	$ap_start16, %ebx
	data32 lgdt (%ebx)

    // 启用保护模式
    movl	%cr0, %eax
    // cr0 的第一位为保护模式位
    orl     $0x1,%eax
    movl	%eax, %cr0

    hlt

    movl	$ap_start_jmp, %eax
	subl	$ap_start16, %eax
	movw	%ax, %bp

// 参考 sipi.h 中的注释
.align	4
.globl sipi_params_16bit
sipi_params_16bit:
ap_start_jmp:
	.long	0
	.word	0
	.word	0
gdtaddr:
	.word	4
	.long	8
	.word	0

ap_start16_end:

// clang-format on
