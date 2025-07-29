/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 栈回溯实现
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

auto backtrace(std::array<uint64_t, kMaxFrameCount> &buffer) -> int {
  auto *rbp = reinterpret_cast<uint64_t *>(cpu_io::Rbp::Read());
  size_t count = 0;
  while ((rbp != nullptr) && (*rbp != 0U) &&
         *rbp >= reinterpret_cast<uint64_t>(__executable_start) &&
         *rbp <= reinterpret_cast<uint64_t>(__etext) &&
         count < buffer.max_size()) {
    auto rip = *(rbp + 1);
    rbp = reinterpret_cast<uint64_t *>(*rbp);
    buffer[count++] = rip;
  }

  return int(count);
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
