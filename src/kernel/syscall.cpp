
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

int32_t sys_putc(char _c) {
    return SYSCALL::get_instance().syscall(SYSCALL::SYS_putc, _c);
}

static int32_t sys_putc(uintptr_t *_arg) {
    char c = (char)_arg[0];
    //    IO::get_instance().put_char(c);
    printf("%c", c);
    return 0;
}

int32_t u_env_call_handler(int _argc, char **_argv) {
    (void)_argc;
    (void)_argv;
#if defined(__riscv)
    assert(_argc == 1);
    CPU::all_regs_t *regs = (CPU::all_regs_t *)_argv[0];
    uintptr_t        arg[8];
    // a0 寄存器保存了系统调用编号
    uint8_t sysno  = regs->xregs.a0;
    arg[0]         = regs->xregs.a1;
    arg[1]         = regs->xregs.a2;
    arg[2]         = regs->xregs.a3;
    arg[3]         = regs->xregs.a4;
    arg[4]         = regs->xregs.a5;
    arg[5]         = regs->xregs.a6;
    arg[6]         = regs->xregs.a7;
    regs->xregs.a0 = SYSCALL::get_instance().do_syscall(sysno, arg);
#endif
    return 0;
}

SYSCALL::syscall_handler_t SYSCALL::syscalls[SYSCALL_NO_MAX] = {
    [SYSCALL::SYS_putc] = sys_putc,
};

int32_t SYSCALL::do_syscall(uint8_t _no, uintptr_t *_argv) {
    // 防止 syscalls[sysno] 越界
    assert(_no >= 0);
    assert(_no < SYSCALL::SYSCALL_NO_MAX);
    if (syscalls[_no] != nullptr) {
        return syscalls[_no](_argv);
    }
    else {
        assert(0);
        return -1;
    }
}

SYSCALL &SYSCALL::get_instance(void) {
    static SYSCALL syscall;
    return syscall;
}

int32_t SYSCALL::init(void) {
#if defined(__riscv)
    INTR::get_instance().register_excp_handler(CPU::EXCP_ECALL_U,
                                               u_env_call_handler);

    INTR::get_instance().register_excp_handler(CPU::EXCP_BREAKPOINT,
                                               u_env_call_handler);
#endif
    info("syscall init.\n");
    return 0;
}

int32_t SYSCALL::init_other_core(void) {
#if defined(__riscv)
    INTR::get_instance().register_excp_handler(CPU::EXCP_ECALL_U,
                                               u_env_call_handler);

    INTR::get_instance().register_excp_handler(CPU::EXCP_BREAKPOINT,
                                               u_env_call_handler);
#endif
    info("syscall other 0x%X init.\n", CPU::get_curr_core_id());
    return 0;
}

int32_t SYSCALL::syscall(uint8_t _sysno, ...) {
    (void)_sysno;
#if defined(__riscv)
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
    asm volatile("ecall"
                 : "+r"(a0)
                 : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a5), "r"(a6), "r"(a7)
                 : "memory");
    return a0;
#else
    return 0;
#endif
}
