
/**
 * @file boot.S
 * @brief boot S
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-07-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-07-15<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#include "macro.S"

#define ASM_FILE
#include "multiboot2.h"

// clang-format off

// multiboot2 文件头
// 计算头长度
.SET HEADER_LENGTH, multiboot_header_end - multiboot_header
// 计算校验和
.SET CHECKSUM, -(MULTIBOOT2_HEADER_MAGIC + MULTIBOOT2_ARCHITECTURE_I386 + HEADER_LENGTH)
// 8 字节对齐
.align MULTIBOOT_HEADER_ALIGN
// 声明所属段
//.section .text.boot
multiboot_header:
    // 魔数
    .long MULTIBOOT2_HEADER_MAGIC
    // 架构
    .long MULTIBOOT2_ARCHITECTURE_I386
    // 头长度
    .long HEADER_LENGTH
    // 校验和
    .long CHECKSUM
    // 添加其它内容在此，详细信息见 Multiboot2 Specification version 2.0.pdf
	.short MULTIBOOT_HEADER_TAG_END
    // 结束标记
    .short 0
    .long 8
multiboot_header_end:

.section .text.boot
.global _boot
.type _boot, @function
.extern _start
#if USE_NO_RELAX == 0
.extern __global_pointer$
#endif
_boot:
    // Check if dtb address (a1) is zero
    beqz a1, 2f
#if USE_NO_RELAX == 0
/// @see riscv-abi.pdf#9.1.4
/// https://github.com/riscv-non-isa/riscv-elf-psabi-doc/releases/tag/v1.0
    // 初始化 gp 寄存器
    // 如果不初始化，则需要在编译时添加 -mno-relax 选项
.option push
.option norelax
1:  auipc gp, %pcrel_hi(__global_pointer$)
    addi  gp, gp, %pcrel_lo(1b)
.option pop
#endif

2:  // 按照每个 core 设置栈地址
    add t0, a0, 1
    slli t0, t0, 12
    la sp, stack_top
    add sp, sp, t0

    // 将 a0 的值写入 tp
    mv tp, a0
    // 保存 sbi 传递的参数
    // 开辟栈空间
    addi sp, sp, -8*2
    // a0 为启动核 id
    SdBase a0, 0, sp
    // a1 为 dtb 地址
    SdBase a1, 1, sp

    // 跳转到 C 代码执行
    call _start

    wfi

// 声明所属段
.section .bss.boot
// 16 字节对齐
.align 16
.global stack_top
stack_top:
    // 跳过 16KB
    .space 4096 * 4

// clang-format on
