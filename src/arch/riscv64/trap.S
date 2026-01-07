/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "macro.S"

.section .text
.global trap_entry
.type trap_entry, @function
.extern HandleTrap

/**
 * @brief Trap 处理入口函数
 *
 * @details 当发生中断或异常时，CPU 跳转到此地址执行。
 * 主要功能：
 * 1. 保存当前 CPU 上下文 (SaveTrapContext)。
 * 2. 将当前栈指针 (sp) 作为参数传递给 HandleTrap。
 * 3. 调用 C++ 实现的 Trap 处理函数 HandleTrap。
 * 4. 恢复 CPU 上下文 (RestoreTrapContext)。
 * 5. 执行 sret 指令返回。
 */
trap_entry:
    // 保存 Trap 上下文
    SaveTrapContext

    // 将栈指针 sp 作为参数传递给 HandleTrap
    mv a0, sp
    // 调用 C++ 中的 Trap 处理函数
    call HandleTrap

    // 恢复 Trap 上下文
    RestoreTrapContext
    // 从 S 模式异常中返回
    sret
