/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "macro.S"

.section .text
.global switch_to
.type switch_to, @function

/**
 * @brief 线程上下文切换
 *
 * void switch_to(CalleeSavedContext* prev, CalleeSavedContext* next);
 *
 * @param prev a0: 当前线程上下文结构体的地址
 * @param next a1: 下一个线程上下文结构体的地址
 */
switch_to:
    // 保存 Callee-saved 寄存器到 prev (a0) 指向的结构体
    SaveCalleeSavedContext a0

    // 从 next (a1) 指向的结构体恢复 Callee-saved 寄存器
    RestoreCalleeSavedContext a1

    ret
