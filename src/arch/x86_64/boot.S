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
    jmpl    $8, $(ap_start32)

// 临时使用的 GDT 描述符
.align 8
gdt:
    // 空描述符 (0x00)
    .quad 0x0000000000000000
    // 32位统一段 (0x08) - 代码和数据共用
    .quad 0x00CF9A000000FFFF
    // 64位统一段 (0x10) - 代码和数据共用
    .quad 0x00AF9A000000FFFF

gdtdesc:
    .word (gdtdesc - gdt - 1)
    .long gdt

ap_start16_end:

.code32
.align 16
.globl ap_start32
ap_start32:
    // 启用 PAE (Physical Address Extension)
    mov %cr4, %eax
    // 设置 PAE 位 (位 5)
    or $0x20, %eax         
    mov %eax, %cr4
    
    // 设置页表 - 使用 sipi_params 中的 cr3 值
    mov (cr3), %eax      
    mov %eax, %cr3     

    hlt
    
    // 启用长模式 (设置 IA32_EFER.LME)
    mov $0xC0000080, %ecx  
    rdmsr
    or $0x100, %eax        
    wrmsr
    
    // 启用分页以激活长模式
    mov %cr0, %eax
    or $0x80000000, %eax   
    mov %eax, %cr0
    
    // 现在处于兼容模式，需要跳转到64位代码段
    ljmp $0x10, $ap_start64

.align	4
.globl	sipi_params
sipi_params:
cr0:
    .long 0
cr3:
	.long 0
argc:
	.long 0
argv:
	.long 0

ap_start32_end:

.code64
.align 16
.globl ap_start64
.globl ap_start64_end
ap_start64:
    call _boot
ap_start64_end:

// clang-format on
