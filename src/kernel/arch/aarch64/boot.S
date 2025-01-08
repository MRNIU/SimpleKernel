
/**
 * @file boot.S
 * @brief boot S
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2024-12-30
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2024-12-30<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

// clang-format off

#define ASM_FILE
#include "multiboot2.h"

// multiboot2 文件头
// 计算头长度
.SET HEADER_LENGTH, multiboot_header_end - multiboot_header
// 计算校验和
.SET CHECKSUM, -(MULTIBOOT2_HEADER_MAGIC + MULTIBOOT2_ARCHITECTURE_I386 + HEADER_LENGTH)
// 8 字节对齐
.align MULTIBOOT_HEADER_ALIGN
// 声明所属段
.section .text.boot
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
_boot:
    // 获取启动核 ID
    mrs x10, mpidr_el1
    and x10, x10, #0xFF
    add x10, x10, #1

    // 按照每个 core 设置栈地址
    lsl x10, x10, #12

    // 按照每个 core 设置栈地址
    ldr x11, =stack_top
    add x11, x11, x10
    mov sp, x11

    // 保存传递的参数（假设传递的参数在 x0 和 x1 中）
    // 开辟栈空间
    stp x0, x1, [sp, #-16]!  // 保存 x0 和 x1 到栈中

    bl _start
    b .

// 声明所属段
.section .bss.boot
// 16 字节对齐
.align 16
.global stack_top
stack_top:
    // 跳过 16KB
    .space 4096 * 4

// clang-format on
