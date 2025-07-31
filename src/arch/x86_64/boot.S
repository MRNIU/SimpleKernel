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

    // 跳转到保护模式
    movl	$ap_start_jmp, %eax
	subl	$ap_start16, %eax
	movw	%ax, %bp
    // 执行远跳转到保护模式，BP 指向包含目标地址和段选择子的内存位置
    hlt
    //data32 cs	ljmp	*(%bp)
    //ljmpl    $(1<<3), $(ap_start)

// 参考 sipi.h 中的注释
.align	4
.globl sipi_params_16bit
sipi_params_16bit:
ap_start_jmp:
	.long	0
	.word	0
	.word	0
gdtaddr:
	.word	0
	.long	0
	.word	0

ap_start16_end:


.code32
.align 16
.globl ap_start
ap_start:
    hlt
	movl $0x233, %eax


	/* This matches struct sipi_param */
	.align	4
.globl	sipi_params
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
