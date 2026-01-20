/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_fdt.hpp"
#include "per_cpu.hpp"
#include "sk_cstdio"
#include "sk_stdlib.h"
#include "virtual_memory.hpp"

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

  fdt_addr = strtoull(argv[2], nullptr, 16);

  core_count = Singleton<KernelFdt>::GetInstance().GetCoreCount();

  interval = cpu_io::CNTFRQ_EL0::Read();
}

void ArchInit(int argc, const char** argv) {
  Singleton<KernelFdt>::GetInstance() =
      KernelFdt(strtoull(argv[2], nullptr, 16));

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr);

  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  Singleton<KernelFdt>::GetInstance().CheckPSCI();

  klog::Info("Hello aarch64 ArchInit\n");
}

void ArchInitSMP(int, const char**) {}

void WakeUpOtherCores() {
  for (size_t i = 0; i < Singleton<BasicInfo>::GetInstance().core_count; i++) {
    auto ret = cpu_io::psci::CpuOn(i, reinterpret_cast<uint64_t>(_boot), 0);
    if ((ret != cpu_io::psci::SUCCESS) && (ret != cpu_io::psci::ALREADY_ON)) {
      klog::Warn("hart %d start failed: %d\n", i, ret);
    }
  }
}
