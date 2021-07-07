
// This file is a part of Simple-XX/SimpleKernel
// (https://github.com/Simple-XX/SimpleKernel).
// Based on http://wiki.0xffffff.org/posts/hurlex-kernel.html
// intr.cpp for Simple-XX/SimpleKernel.

#include "cpu.hpp"
#include "stdio.h"
#include "intr.h"
#include "cpu.hpp"

namespace INTR {
    int32_t init(void) {
        // 内部中断初始化
        CLINT::init();
        // 外部中断初始化
        PLIC::init();
        printf("intr init\n");
        return 0;
    }

    void handler_default(void) {
        while (1) {
            ;
        }
        return;
    }

    void trap_handler(uint64_t _scause, uint64_t _sepc) {
// #define DEBUG
#ifdef DEBUG
        printf("scause: 0x%X, sepc: 0x%X\n", _scause, _sepc);
#undef DEBUG
#endif
        if (_scause & CPU::CAUSE_INTR_MASK) {
// 中断
// #define DEBUG
#ifdef DEBUG
            printf("intr: %s\n",
                   CLINT::intr_names[_scause & CPU::CAUSE_CODE_MASK]);
#undef DEBUG
#endif
            // 跳转到对应的处理函数
            CLINT::do_interrupt(_scause & CPU::CAUSE_CODE_MASK);
        }
        else {
// 异常
// 跳转到对应的处理函数
// #define DEBUG
#ifdef DEBUG
            printf("excp: %s\n",
                   CLINT::excp_names[_scause & CPU::CAUSE_CODE_MASK]);
#undef DEBUG
#endif
            CLINT::do_excp(_scause & CPU::CAUSE_CODE_MASK);
        }
        return;
    }
};