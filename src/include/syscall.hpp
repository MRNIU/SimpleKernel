/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SYSCALL_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SYSCALL_HPP_

#include <cpu_io.h>

#include <cstddef>
#include <cstdint>

// 参考 Linux 系统调用号
#if defined(__riscv) || defined(__aarch64__)
// RISC-V 64 和 AArch64 使用 asm-generic 编号
static constexpr const uint64_t SYSCALL_WRITE = 64;
static constexpr const uint64_t SYSCALL_EXIT = 93;
static constexpr const uint64_t SYSCALL_YIELD = 124;
#elif defined(__x86_64__)
// x86_64 使用自己的编号
static constexpr const uint64_t SYSCALL_WRITE = 1;
static constexpr const uint64_t SYSCALL_EXIT = 60;
static constexpr const uint64_t SYSCALL_YIELD = 24;
#else
#error "Unsupported architecture for syscall numbers"
#endif

// 由各个架构实现
void Syscall(uint64_t cause, cpu_io::TrapContext* context);

int syscall_dispatcher(int64_t syscall_id, uint64_t args[6]);

int sys_write(int fd, const char* buf, size_t len);
int sys_exit(int code);
int sys_yield();
int sys_sleep(uint64_t ms);

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SYSCALL_HPP_ */
