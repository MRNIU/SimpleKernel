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

// printf_bare_metal 基本输出实现
extern "C" void putchar_(char character) {
  sbi_debug_console_write_byte(character);
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
  /// @todo 这个大小会不会丢掉 elf 信息？
  elf_size = kernel_size;

  fdt_addr = reinterpret_cast<uint64_t>(argv);

  core_count = Singleton<KernelFdt>::GetInstance().GetCoreCount();
}

void ArchInit(int argc, const char **argv) {
  // Print out argc value
  klog::Info("ArchInit: argc = %d\n", argc);

  // Print out all argv values
  for (int i = 0; i < argc; i++) {
    klog::Info("ArchInit: argv[%d] = %p (%s)\n", i, argv[i], argv[i]);
  }
  Singleton<KernelFdt>::GetInstance() =
      KernelFdt(reinterpret_cast<uint64_t>(argv));

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr);

  auto [serial_base, serial_size] =
      Singleton<KernelFdt>::GetInstance().GetSerial();
  auto uart = Ns16550a(serial_base);
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
