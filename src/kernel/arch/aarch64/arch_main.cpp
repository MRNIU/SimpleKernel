
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

// printf_bare_metal 基本输出实现
namespace {
Pl011* pl011 = nullptr;
}
extern "C" void _putchar(char character) {
  if (pl011) {
    pl011->PutChar(character);
  }
}

BasicInfo::BasicInfo(uint32_t argc, const uint8_t* argv) {
  (void)argc;
  (void)argv;

  auto [memory_base, memory_size] =
      Singleton<KernelFdt>::GetInstance().GetMemory();
  physical_memory_addr = memory_base;
  physical_memory_size = memory_size;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);
  elf_addr = 0;
  elf_size = 0;

  fdt_addr = 0x40000000;
}

void ArchInit(uint32_t argc, const uint8_t* argv) {
  // 初始化 FPU
  cpu_io::SetupFpu();

  Singleton<KernelFdt>::GetInstance() = KernelFdt(0x40000000);

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  Singleton<BasicInfo>::GetInstance().core_count++;

  auto [serial_base, serial_size] =
      Singleton<KernelFdt>::GetInstance().GetSerial();

  static auto uart = Pl011(serial_base);
  pl011 = &uart;
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

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

  // __asm__ volatile("sev");
}

void ArchInitSMP(uint32_t argc, const uint8_t* argv) {
  (void)argc;
  (void)argv;

  Singleton<BasicInfo>::GetInstance().core_count++;
}
