
/**
 * @file arch_main.cpp
 * @brief arch_main cpp
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

#include "arch.h"
#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_fdt.hpp"
#include "per_cpu.hpp"
#include "pl011.h"
#include "sk_cstdio"
#include "sk_libc.h"

// printf_bare_metal 基本输出实现
namespace {
Pl011 *pl011 = nullptr;
}
extern "C" void putchar_(char character) {
  if (pl011) {
    pl011->PutChar(character);
  }
}

extern "C" void *sizeof_dtb[];

BasicInfo::BasicInfo(int argc, const char **argv) {
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

void ArchInit(int argc, const char **argv) {
  // 初始化 FPU
  // cpu_io::SetupFpu();

  Singleton<KernelFdt>::GetInstance() =
      KernelFdt(strtoull(argv[2], nullptr, 16));

  auto [serial_base, serial_size] =
      Singleton<KernelFdt>::GetInstance().GetSerial();

  static auto uart = Pl011(serial_base);
  pl011 = &uart;

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr);

  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  Singleton<KernelFdt>::GetInstance().CheckPSCI();

  klog::Info("serial_base: 0x%lx\n", serial_base);

  uart.PutChar('H');
  uart.PutChar('e');
  uart.PutChar('l');
  uart.PutChar('l');
  uart.PutChar('o');
  uart.PutChar(' ');
  uart.PutChar('u');
  uart.PutChar('a');
  uart.PutChar('r');
  uart.PutChar('t');
  uart.PutChar('!');
  uart.PutChar('\n');

  klog::Info("Hello aarch64 ArchInit\n");

  // 唤醒其余 core
  for (size_t i = 0; i < Singleton<BasicInfo>::GetInstance().core_count; i++) {
    auto ret = cpu_io::psci::CpuOn(i, reinterpret_cast<uint64_t>(_boot), 0);
    if ((ret != cpu_io::psci::SUCCESS) && (ret != cpu_io::psci::ALREADY_ON)) {
      klog::Warn("hart %d start failed: %d\n", i, ret);
    }
  }
}

void ArchInitSMP(int, const char **) {
  // 初始化 FPU
  // cpu_io::SetupFpu();

  putchar_('!');
  printf("12345\n");
}
