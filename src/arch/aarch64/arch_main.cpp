/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief arch_main cpp
 */

#include <cpu_io.h>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_fdt.hpp"
#include "per_cpu.hpp"
#include "pl011.h"
#include "sk_cstdio"
#include "sk_libc.h"
#include "virtual_memory.hpp"

// 基本输出实现
namespace {
Pl011* pl011 = nullptr;
}
extern "C" void sk_putchar(int c, [[maybe_unused]] void* ctx) {
  if (pl011) {
    pl011->PutChar(c);
  }
}

BasicInfo::BasicInfo(int argc, const char** argv) {
  (void)argc;
  (void)argv;

  auto [memory_base, memory_size] =
      Singleton<KernelFdt>::GetInstance().GetMemory();
  physical_memory_addr = memory_base;
  physical_memory_size = memory_size;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);
  elf_addr = kernel_addr;
  elf_size = kernel_size;

  fdt_addr = strtoull(argv[2], nullptr, 16);

  core_count = Singleton<KernelFdt>::GetInstance().GetCoreCount();
}

void ArchInit(int argc, const char** argv) {
  Singleton<KernelFdt>::GetInstance() =
      KernelFdt(strtoull(argv[2], nullptr, 16));

  auto [serial_base, serial_size, irq] =
      Singleton<KernelFdt>::GetInstance().GetSerial();

  Singleton<Pl011>::GetInstance() = Pl011(serial_base);
  pl011 = &Singleton<Pl011>::GetInstance();

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr);

  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  Singleton<KernelFdt>::GetInstance().CheckPSCI();

  klog::Info("Hello aarch64 ArchInit\n");

  // 唤醒其余 core
  for (size_t i = 0; i < Singleton<BasicInfo>::GetInstance().core_count; i++) {
    auto ret = cpu_io::psci::CpuOn(i, reinterpret_cast<uint64_t>(_boot), 0);
    if ((ret != cpu_io::psci::SUCCESS) && (ret != cpu_io::psci::ALREADY_ON)) {
      klog::Warn("hart %d start failed: %d\n", i, ret);
    }
  }
}

void ArchInitSMP(int, const char**) {}

void ArchReMap() {
  // 映射串口
  auto [serial_base, serial_size, irq] =
      Singleton<KernelFdt>::GetInstance().GetSerial();
  // Singleton<VirtualMemory>::GetInstance().MapMMIO(serial_base, serial_size);
  // Singleton<VirtualMemory>::GetInstance().MapMMIO(0x9040000, 0x1000);
  Singleton<VirtualMemory>::GetInstance().MapMMIO(0x9000000, 0x1000);
}
