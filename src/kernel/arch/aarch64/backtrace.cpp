/**
 * @file backtrace.cpp
 * @brief 栈回溯实现
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-07-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-07-15<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#include <cpu_io.h>

#include <array>

#include "arch.h"
#include "kernel_elf.hpp"
#include "sk_cstdio"
#include "sk_libc.h"

auto backtrace(void **buffer, int size) -> int {
  auto *x29 = reinterpret_cast<uint64_t *>(cpu::regs::X29::Read());

  int count = 0;
  while ((x29 != nullptr) && (*x29 != 0U) && count < size) {
    uint64_t lr = x29[1];
    x29 = reinterpret_cast<uint64_t *>(x29[0]);
    buffer[count++] = reinterpret_cast<void *>(lr);
  }

  return count;
}

void DumpStack() {
  std::array<void *, kMaxFrameCount> buffer{};

  // 获取调用栈中的地址
  auto num_frames = backtrace(buffer.data(), kMaxFrameCount);

  // 打印地址
  /// @todo 打印函数名，需要 elf 支持
  for (auto i = 0; i < num_frames; i++) {
    printf("[0x%p]\n", buffer[i]);
  }
}
