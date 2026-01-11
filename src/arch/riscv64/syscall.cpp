/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "syscall.hpp"

#include "interrupt.h"
#include "kernel_log.hpp"
#include "singleton.hpp"

void Syscall(uint64_t, uint8_t* context_ptr) {
  // 获取系统调用号和参数
  auto ctx = reinterpret_cast<cpu_io::TrapContext*>(context_ptr);
  uint64_t syscall_id = 0;
  uint64_t args[6] = {0};

  syscall_id = ctx->a7;
  args[0] = ctx->a0;
  args[1] = ctx->a1;
  args[2] = ctx->a2;
  args[3] = ctx->a3;
  args[4] = ctx->a4;
  args[5] = ctx->a5;

  // 执行处理函数
  auto ret = syscall_dispatcher(syscall_id, args);

  // 设置返回值
  ctx->a0 = static_cast<uint64_t>(ret);
  // 跳过 ecall 指令
  ctx->sepc += 4;
}
