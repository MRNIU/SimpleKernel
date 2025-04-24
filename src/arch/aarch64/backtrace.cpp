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
#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "singleton.hpp"

__always_inline auto backtrace(std::array<uint64_t, kMaxFrameCount> &buffer)
    -> int {
  auto *x29 = reinterpret_cast<uint64_t *>(cpu_io::X29::Read());
  size_t count = 0;
  while ((x29 != nullptr) && (x29[0] != 0U) &&
         x29[0] >= reinterpret_cast<uint64_t>(__executable_start) &&
         x29[0] <= reinterpret_cast<uint64_t>(__etext) &&
         count < buffer.max_size()) {
    auto lr = x29[1];
    x29 = reinterpret_cast<uint64_t *>(x29[0]);
    buffer[count++] = lr;
  }

  return int(count);
}

void DumpStack() {
  std::array<uint64_t, kMaxFrameCount> buffer{};

  // 获取调用栈中的地址
  auto num_frames = backtrace(buffer);

  // 打印函数名
  for (auto current_frame_idx = 0; current_frame_idx < num_frames;
       current_frame_idx++) {
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
