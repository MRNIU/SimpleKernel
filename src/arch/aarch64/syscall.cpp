/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "syscall.hpp"

#include "interrupt.h"
#include "kernel_log.hpp"
#include "singleton.hpp"
#include "task.hpp"

void Syscall(uint64_t, uint8_t* context_ptr) {
  auto ctx = reinterpret_cast<cpu_io::TrapContext*>(context_ptr);

  // 1. 获取系统调用号和参数
  uint64_t syscall_id = 0;
  uint64_t args[6] = {0};

#if defined(__riscv)
  syscall_id = ctx->a7;
  args[0] = ctx->a0;
  args[1] = ctx->a1;
  args[2] = ctx->a2;
  args[3] = ctx->a3;
  args[4] = ctx->a4;
  args[5] = ctx->a5;
#elif defined(__aarch64__)
  // 假设 AArch64 TrapContext 包含 x0-x30
  // Linux 约定: x8 是系统调用号
  syscall_id = ctx->x8;
  args[0] = ctx->x0;
  args[1] = ctx->x1;
  args[2] = ctx->x2;
  args[3] = ctx->x3;
  args[4] = ctx->x4;
  args[5] = ctx->x5;
#elif defined(__x86_64__)
  // 假设 x86_64 TrapContext 包含 rax, rdi, rsi 等
  // Linux 约定: rax 是系统调用号
  syscall_id = ctx->rax;
  args[0] = ctx->rdi;
  args[1] = ctx->rsi;
  args[2] = ctx->rdx;
  args[3] = ctx->r10;
  args[4] = ctx->r8;
  args[5] = ctx->r9;
#endif

  // 2. 执行处理函数
  int64_t ret = 0;
  switch (syscall_id) {
    case SYSCALL_WRITE:
      ret = sys_write(static_cast<int>(args[0]),
                      reinterpret_cast<const char*>(args[1]),
                      static_cast<size_t>(args[2]));
      break;
    case SYSCALL_EXIT:
      ret = sys_exit(static_cast<int>(args[0]));
      break;
    case SYSCALL_YIELD:
      ret = sys_yield();
      break;
    default:
      klog::Err("[Syscall] Unknown syscall id: %d\n", syscall_id);
      ret = -1;
      break;
  }

    // 3. 设置返回值 并 调整 PC (跳过 ecall/svc 指令)
#if defined(__riscv)
  ctx->a0 = static_cast<uint64_t>(ret);
  ctx->sepc += 4;  // 跳过 ecall 指令
#elif defined(__aarch64__)
  ctx->x0 = static_cast<uint64_t>(ret);
  ctx->elr_el1 += 4;  // 跳过 svc 指令 (假设是 svc 0 且处于 AArch64 模式)
#elif defined(__x86_64__)
  ctx->rax = static_cast<uint64_t>(ret);
  // 这里的处理取决于你是用 int 0x80 还是 syscall 指令进入的。
  // 如果是 syscall 指令，硬件通常将返回地址保存在 rcx 中，不需要修改 rip。
  // 如果是中断门，rip 指向 int 指令下一条，也不需要修改。
  // 只有在某些 Trap 机制下 rip 指向产生异常的指令本身时才需要加。
#endif
}
