
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
#include "intr.h"

inline int32_t syscall(uint8_t _sysno, ...) {
    va_list ap;
    va_start(ap, _sysno);
    uintptr_t a[MAX_ARGS];
    for (size_t i = 0; i < MAX_ARGS; i++) {
        a[i] = va_arg(ap, uintptr_t);
    }
    va_end(ap);
    register uintptr_t a0 asm("a0") = _sysno;
    register uintptr_t a1 asm("a1") = a[0];
    register uintptr_t a2 asm("a2") = a[1];
    register uintptr_t a3 asm("a3") = a[2];
    register uintptr_t a4 asm("a4") = a[3];
    register uintptr_t a5 asm("a5") = a[4];
    register uintptr_t a6 asm("a6") = a[5];
    register uintptr_t a7 asm("a7") = a[6];
    asm volatile("ebreak"
                 : "+r"(a0)
                 : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a5), "r"(a6), "r"(a7)
                 : "memory");
    return a0;
}

int32_t sys_putc(int _c) {
    return syscall(SYS_putc, _c);
}

static int32_t sys_putc(uintptr_t *_arg) {
    int c = (int)_arg[0];
    IO::get_instance().put_char(c);
    return 0;
}

//这里定义了函数指针的数组syscalls,
//把每个系统调用编号的下标上初始化为对应的函数指针
int32_t (*syscalls[1])(uintptr_t *) = {sys_putc};

#define NUM_SYSCALLS ((sizeof(syscalls)) / (sizeof(syscalls[0])))

int32_t u_env_call_hancler(int _argc, char **_argv) {
    assert(_argc == 1);
    info("u_env_call_hancler\n");
    CPU::all_regs_t *regs = (CPU::all_regs_t *)_argv[0];
    uintptr_t        arg[8];
    // a0 寄存器保存了系统调用编号
    int sysno = regs->xregs.a0;
    // 防止syscalls[sysno]下标越界
    if (sysno >= 0 && sysno < NUM_SYSCALLS) {
        if (syscalls[sysno] != NULL) {
            arg[0]         = regs->xregs.a1;
            arg[1]         = regs->xregs.a2;
            arg[2]         = regs->xregs.a3;
            arg[3]         = regs->xregs.a4;
            arg[4]         = regs->xregs.a5;
            arg[5]         = regs->xregs.a6;
            arg[6]         = regs->xregs.a7;
            regs->xregs.a0 = syscalls[sysno](arg);
            return 0;
        }
    }
    // 如果执行到这里，说明传入的系统调用编号还没有被实现，就崩掉了。
    assert(0);
    return 0;
}

int32_t syscall_init(void) {
    INTR::get_instance().register_excp_handler(INTR::EXCP_U_ENV_CALL,
                                               u_env_call_hancler);

    INTR::get_instance().register_excp_handler(INTR::EXCP_BREAK,
                                               u_env_call_hancler);
    info("syscall init.\n");
    return 0;
}
