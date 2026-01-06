/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "macro.S"

.section .text
.global SwitchTo
.type SwitchTo, @function

/**
 * @brief 线程上下文切换
 *
 * @param prev a0: 保存当前线程上下文指针的地址 (CalleeSavedContext** prev)
 * @param next a1: 下一个线程的上下文指针 (CalleeSavedContext* next)
 */
SwitchTo:
    // 在当前栈上分配空间保存 Callee-saved 寄存器
    SaveCalleeSavedContext

    // 保存当前 sp 到 *prev
    sd sp, (a0)

    // 切换到新栈
    mv sp, a1

    // 恢复寄存器
    RestoreCalleeSavedContext

    ret
