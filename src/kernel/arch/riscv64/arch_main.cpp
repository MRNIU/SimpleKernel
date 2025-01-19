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
#include "kernel_elf.hpp"
#include "kernel_fdt.hpp"
#include "ns16550a.h"
#include "per_cpu.hpp"
#include "sk_cstdio"
#include "sk_libc.h"

// printf_bare_metal 基本输出实现
extern "C" void _putchar(char character) {
  sbi_debug_console_write_byte(character);
}

BasicInfo::BasicInfo(uint32_t argc, const uint8_t *argv) {
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

  fdt_addr = reinterpret_cast<uint64_t>(argv);
}

void ArchInit(uint32_t argc, const uint8_t *argv) {
  printf("boot hart id: %d\n", argc);
  printf("dtb info addr: %p\n", argv);

  // 将 core id 保存到 tp 寄存器
  cpu_io::Tp::Write(argc);
  GetCurrentCore().core_id_ = argc;

  Singleton<KernelFdt>::GetInstance() =
      KernelFdt(reinterpret_cast<uint64_t>(argv));

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  Singleton<BasicInfo>::GetInstance().core_count++;
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

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

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() = KernelElf();

  klog::Info("Hello riscv64 ArchInit\n");
  for (size_t i = 0; i < PerCpu::kMaxCoreCount; i++) {
    auto ret = sbi_hart_start(i, reinterpret_cast<uint64_t>(_boot), 0);
    if ((ret.error != SBI_SUCCESS) &&
        (ret.error != SBI_ERR_ALREADY_AVAILABLE)) {
      printf("hart %d start failed: %d\n", i, ret.error);
    }
  }
}

void ArchInitSMP(uint32_t argc, const uint8_t *) {
  cpu_io::Tp::Write(argc);
  GetCurrentCore().core_id_ = argc;
  Singleton<BasicInfo>::GetInstance().core_count++;
}
