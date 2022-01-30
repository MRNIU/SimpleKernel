
/**
 * @file syscall.cpp
 * @brief 系统调用实现
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2022-01-30
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2022-01-30<td>MRNIU<td>创建文件
 * </table>
 */

#include "stdarg.h"
#include "syscall.h"
#include "stdint.h"
#include "io.h"
#include "cpu.hpp"
#include "task.h"
#include "core.h"
#include "intr.h"

#define MAX_ARGS 5
static inline int syscall(int num, ...) {
    // va_list, va_start, va_arg都是C语言处理参数个数不定的函数的宏
    //在stdarg.h里定义
    va_list ap;        // ap: 参数列表(此时未初始化)
    va_start(ap, num); //初始化参数列表, 从num开始
    // First, va_start initializes the list of variable arguments as a va_list.
    uint64_t a[MAX_ARGS];
    int      i, ret;
    for (i = 0; i < MAX_ARGS; i++) { //把参数依次取出
        /*Subsequent executions of va_arg yield the values of the additional
        arguments in the same order as passed to the function.*/
        a[i] = va_arg(ap, uint64_t);
    }
    va_end(ap); // Finally, va_end shall be executed before the function
                // returns.
    asm volatile("ld a0, %1\n"
                 "ld a1, %2\n"
                 "ld a2, %3\n"
                 "ld a3, %4\n"
                 "ld a4, %5\n"
                 "ld a5, %6\n"
                 "ecall\n"
                 "sd a0, %0"
                 : "=m"(ret)
                 : "m"(num), "m"(a[0]), "m"(a[1]), "m"(a[2]), "m"(a[3]),
                   "m"(a[4])
                 : "memory");
    // num存到a0寄存器， a[0]存到a1寄存器
    // ecall的返回值存到ret
    return ret;
}

int sys_putc(int c) {
    info("ccccccc\n");
    return syscall(SYS_putc, c);
}

// void syscall(void) {
//     task_t  *task = core_t::get_curr_task();
//     uint64_t arg[5];
//     int      num = task->context.a0; // a0寄存器保存了系统调用编号
//     if (num >= 0 && num < NUM_SYSCALLS) { //防止syscalls[num]下标越界
//         if (syscalls[num] != NULL) {
//             arg[0]     = task->context.a1;
//             arg[1]     = task->context.a2;
//             arg[2]     = task->context.a3;
//             arg[3]     = task->context.a4;
//             arg[4]     = task->context.a5;
//             tf->gpr.a0 = syscalls[num](arg);
//             //把寄存器里的参数取出来，转发给系统调用编号对应的函数进行处理
//             return;
//         }
//     }
//     //如果执行到这里，说明传入的系统调用编号还没有被实现，就崩掉了。
//     print_trapframe(tf);
//     panic("undefined syscall %d, pid = %d, name = %s.\n", num, current->pid,
//           current->name);
// }

static void u_env_call_hancler(void) {
    info("u_env_call_hancler\n");
    return;
}

bool syscall_init(void) {
    INTR::get_instance().register_excp_handler(INTR::EXCP_U_ENV_CALL,
                                               u_env_call_hancler);

    INTR::get_instance().register_excp_handler(INTR::EXCP_S_ENV_CALL,
                                               u_env_call_hancler);
    info("syscall init.\n");
    return true;
}
