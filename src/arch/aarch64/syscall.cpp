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

  syscall_id = ctx->x8;
  args[0] = ctx->x0;
  args[1] = ctx->x1;
  args[2] = ctx->x2;
  args[3] = ctx->x3;
  args[4] = ctx->x4;
  args[5] = ctx->x5;

  // 执行处理函数
  auto ret = syscall_dispatcher(syscall_id, args);

  ctx->x0 = static_cast<uint64_t>(ret);
  ctx->elr_el1 += 4;
}
