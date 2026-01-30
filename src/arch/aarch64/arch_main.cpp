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

BasicInfo::BasicInfo(int, const char** argv) {
  Singleton<KernelFdt>::GetInstance()
      .GetMemory()
      .and_then([this](std::pair<uint64_t, size_t> mem) -> Expected<void> {
        physical_memory_addr = mem.first;
        physical_memory_size = mem.second;
        return {};
      })
      .or_else([](Error err) -> Expected<void> {
        klog::Err("Failed to get memory info: %s\n", err.message());
        while (true) {
          cpu_io::Pause();
        }
        return {};
      });

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);
  elf_addr = kernel_addr;

  fdt_addr = strtoull(argv[2], nullptr, 16);

  core_count = Singleton<KernelFdt>::GetInstance().GetCoreCount().value_or(1);

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

  Singleton<KernelFdt>::GetInstance().CheckPSCI().or_else(
      [](Error err) -> Expected<void> {
        klog::Err("CheckPSCI failed: %s\n", err.message());
        return {};
      });

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
