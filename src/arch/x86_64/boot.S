/**
 * @copyright Copyright The SimpleKernel Contributors
 */

// 每个 CPU 的栈大小：64KB
#define STACK_SIZE_PER_CPU      0x10000
// 最大 CPU 数量
#define MAX_CPU_COUNT           4
// 总栈大小
#define TOTAL_STACK_SIZE        (STACK_SIZE_PER_CPU * MAX_CPU_COUNT)
// CR4 中的 PAE 位 (位 5)
#define PAE_BIT                 0x20
// CR0 中的保护模式位
#define PROTECTION_MODE_BIT     0x1
// IA32_EFER 中的长模式位
#define LONG_MODE_BIT           0x100
// CR0 中的分页位
#define PAGING_BIT              0x80000000
// IA32_EFER MSR 地址
#define IA32_EFER_MSR           0xC0000080
// CPUID leaf 1
#define CPUID_LEAF_1            1
// APIC ID 在 EBX 中的位移
#define APIC_ID_SHIFT           24
// APIC ID 掩码
#define APIC_ID_MASK            0xFF

// clang-format off

// 以下是多核启动入口
.section .text
.global ap_start16
.align 16
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
    or $PROTECTION_MODE_BIT,%eax
    mov %eax, %cr0

    // 跳转到保护模式
    jmpl    $8, $(ap_start32)

.code32
ap_start32:
    // 启用 PAE (Physical Address Extension)
    mov %cr4, %eax
    // 设置 PAE 位 (位 5)
    or $PAE_BIT, %eax
    mov %eax, %cr4

    // 设置页表 - 使用 sipi_params 中的 cr3 值
    // 计算 sipi_params.cr3 的相对地址
    mov $cr3, %ebx
    sub $ap_start16, %ebx
    mov (%ebx), %eax
    mov %eax, %cr3

    // 启用长模式 (设置 IA32_EFER.LME)
    mov $IA32_EFER_MSR, %ecx
    rdmsr
    or $LONG_MODE_BIT, %eax
    wrmsr

    // 启用分页以激活长模式
    mov %cr0, %eax
    or $PAGING_BIT, %eax
    mov %eax, %cr0

    // 现在处于兼容模式，需要跳转到64位代码段
    ljmp $0x10, $_boot

.code64
.global _boot
.type _boot, @function
.extern _start
_boot:
    // 关中断
    cli

    // 读取 CPU ID (APIC ID) 使用 CPUID
    mov $CPUID_LEAF_1, %eax
    cpuid
    // EBX bits 31-24 是初始 APIC ID
    shr $APIC_ID_SHIFT, %ebx
    // 保留低 8 位
    and $APIC_ID_MASK, %ebx

    // 现在 EBX 包含 CPU ID (APIC ID)
    // 根据 CPU ID 计算栈地址: stack_top + CPU_ID * 4KB
    mov %ebx, %eax
    mov $STACK_SIZE_PER_CPU, %edx
    mul %edx

    // 计算该 CPU 的栈顶地址
    mov $stack_top, %rsp
    add %rax, %rsp
    add $STACK_SIZE_PER_CPU, %rsp

    call _start
    hlt
    ret

// 临时使用的 GDT 描述符
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

.globl sipi_params
sipi_params:
cr3:
	.long 0

.globl ap_start64_end
ap_start64_end:

.section .bss
// MAX_CPU_COUNT 个 CPU × STACK_SIZE_PER_CPU 每个 = TOTAL_STACK_SIZE 总栈空间
stack_top:
    .space TOTAL_STACK_SIZE

// clang-format on
