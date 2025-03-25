
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

__always_inline auto backtrace(std::array<uint64_t, kMaxFrameCount> &buffer)
    -> int {
  auto *fp = reinterpret_cast<uint64_t *>(cpu_io::Fp::Read());
  uint64_t ra = 0;

  int count = 0;
  while ((fp != nullptr) && (*fp != 0U) && count < buffer.max_size()) {
    ra = *(fp - 1);
    fp = reinterpret_cast<uint64_t *>(*(fp - 2));
    buffer[count++] = ra;
  }
  return count;
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
