/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief arch_main cpp
 */

#include <cpu_io.h>

#include <cstdint>

#include "apic.h"
#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
#include "singleton.hpp"
#include "sipi.h"
#include "sk_iostream"

// 基本输出实现
namespace {
cpu_io::Serial *serial = nullptr;
}  // namespace
extern "C" void sk_putchar(int c, [[maybe_unused]] void *ctx) {
  if (serial) {
    serial->Write(c);
  }
}

BasicInfo::BasicInfo(int, const char **) {
  physical_memory_addr = 0;
  physical_memory_size = 0;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);

  elf_addr = kernel_addr;
  elf_size = kernel_size;

  fdt_addr = 0;

  core_count = cpu_io::GetCpuTopologyInfo().logical_processors;
}

auto ArchInit(int argc, const char **argv) -> int {
  if (argc != 1) {
    klog::Err("argc != 1 [%d]\n", argc);
    throw;
  }

  Singleton<cpu_io::Serial>::GetInstance() = cpu_io::Serial(cpu_io::kCom1);
  serial = &Singleton<cpu_io::Serial>::GetInstance();

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr,
                Singleton<BasicInfo>::GetInstance().elf_size);

  klog::Info("Hello x86_64 ArchInit\n");

  Singleton<Apic>::GetInstance() = Apic();
  Singleton<Apic>::GetInstance().Init(
      Singleton<BasicInfo>::GetInstance().core_count);
  Singleton<Apic>::GetInstance().InitCurrentCpuLocalApic();

  Singleton<Apic>::GetInstance().PrintInfo();

  // 计算 SMP 启动代码大小
  auto smp_boot_size = reinterpret_cast<size_t>(ap_start64_end) -
                       reinterpret_cast<size_t>(ap_start16);

  klog::Info("SMP boot code: start=%p, end=%p, size=%zu bytes\n", ap_start16,
             ap_start64_end, smp_boot_size);

  // 计算 sipi_params 在目标内存中的实际地址
  auto target_sipi_params = reinterpret_cast<sipi_params_t *>(sipi_params);

  // 填充 sipi_params 结构体
  target_sipi_params->cr3 = cpu_io::Cr3::Read();

  // 唤醒其它 core
  size_t started_aps = Singleton<Apic>::GetInstance().StartupAllAps(
      ap_start16, smp_boot_size, kDefaultAPBase);
  klog::Info("Started %zu Application Processors\n", started_aps);

  return 0;
}

auto ArchInitSMP(int argc, const char **argv) -> int {
  (void)argc;
  (void)argv;
  return 0;
}
