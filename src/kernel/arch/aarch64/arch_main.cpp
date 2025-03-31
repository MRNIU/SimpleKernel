
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
extern "C" void _putchar(char character) {
  if (pl011) {
    pl011->PutChar(character);
  }
}

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
  cpu_io::SetupFpu();

  static auto uart = Pl011(0x9000000);
  pl011 = &uart;

  klog::Info("argc = %d\n", argc);
  for (int i = 0; i < argc; i++) {
    klog::Info("argv[%d] = [%s]\n", i, argv[i]);
  }

  Singleton<KernelFdt>::GetInstance() =
      KernelFdt(strtoull(argv[2], nullptr, 16));

  klog::Info("Singleton<KernelFdt>::GetInstance().GetCoreCount(): %d\n",
             Singleton<KernelFdt>::GetInstance().GetCoreCount());

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);

  auto [serial_base, serial_size] =
      Singleton<KernelFdt>::GetInstance().GetSerial();

  sk_std::cout << Singleton<BasicInfo>::GetInstance();

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
}

void ArchInitSMP(int argc, const char **argv) {
  (void)argc;
  (void)argv;
}
