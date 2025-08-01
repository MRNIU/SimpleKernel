/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief arch_main cpp
 */

#include <cpu_io.h>
#include <opensbi_interface.h>

#include "basic_info.hpp"
#include "config.h"
#include "kernel_elf.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "ns16550a.h"
#include "per_cpu.hpp"
#include "sk_cstdio"
#include "sk_libc.h"

// 基本输出实现
extern "C" void sk_putchar(int c, [[maybe_unused]] void *ctx) {
  sbi_debug_console_write_byte(c);
}

BasicInfo::BasicInfo(int, const char **argv) {
  auto [memory_base, memory_size] =
      Singleton<KernelFdt>::GetInstance().GetMemory();
  physical_memory_addr = memory_base;
  physical_memory_size = memory_size;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);
  elf_addr = kernel_addr;
  elf_size = kernel_size;

  fdt_addr = reinterpret_cast<uint64_t>(argv);

  core_count = Singleton<KernelFdt>::GetInstance().GetCoreCount();
}

void ArchInit(int argc, const char **argv) {
  Singleton<KernelFdt>::GetInstance() =
      KernelFdt(reinterpret_cast<uint64_t>(argv));

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr);

  klog::Info("Hello riscv64 ArchInit\n");

  // 唤醒其余 core
  for (size_t i = 0; i < Singleton<BasicInfo>::GetInstance().core_count; i++) {
    auto ret = sbi_hart_start(i, reinterpret_cast<uint64_t>(_boot), 0);
    if ((ret.error != SBI_SUCCESS) &&
        (ret.error != SBI_ERR_ALREADY_AVAILABLE)) {
      klog::Warn("hart %d start failed: %d\n", i, ret.error);
    }
  }
}

void ArchInitSMP(int, const char **) {}
