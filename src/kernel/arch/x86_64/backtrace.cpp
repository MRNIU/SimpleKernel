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
#include <elf.h>

#include <array>
#include <cerrno>
#include <cstdint>

#include "arch.h"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "singleton.hpp"

auto backtrace(std::array<uint64_t, kMaxFrameCount> &buffer) -> int {
  auto *rbp = reinterpret_cast<uint64_t *>(cpu::regs::Rbp::Read());
  uint64_t *rip = nullptr;

  int count = 0;
  while ((rbp != nullptr) && (*rbp != 0U) && count < buffer.max_size()) {
    rip = rbp + 1;
    rbp = reinterpret_cast<uint64_t *>(*rbp);
    buffer[count++] = *rip;
  }

  return count;
}

void DumpStack() {
  std::array<uint64_t, kMaxFrameCount> buffer{};

  // 获取调用栈中的地址
  auto num_frames = backtrace(buffer);

  for (auto current_frame_idx = 0; current_frame_idx < num_frames;
       current_frame_idx++) {
    // 打印函数名
    for (auto symtab : Singleton<KernelElf>::GetInstance().symtab_) {
      if ((ELF64_ST_TYPE(symtab.st_info) == STT_FUNC) &&
          (buffer[current_frame_idx] >= symtab.st_value) &&
          (buffer[current_frame_idx] <= symtab.st_value + symtab.st_size)) {
        klog::Err("[%s] 0x%p\n",
                  Singleton<KernelElf>::GetInstance().strtab_ + symtab.st_name,
                  buffer[current_frame_idx]);
      }
    }
  }
}
