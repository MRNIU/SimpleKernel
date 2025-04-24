
/**
 * @file main.cpp
 * @brief 用于测试 gnu-efi 启动
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

#include <cstdint>

#include "basic_info.hpp"
#include "sk_cstdio"
#include "sk_cstring"

extern "C" void putchar_(char character) {
  auto serial = cpu_io::Serial(cpu_io::kCom1);
  serial.Write(character);
}

// 引用链接脚本中的变量
/// @see http://wiki.osdev.org/Using_Linker_Script_Values
/// 内核开始
extern "C" void *__executable_start[];  // NOLINT
/// 内核结束
extern "C" void *end[];
BasicInfo::BasicInfo(uint32_t argc, const uint8_t *argv) {
  (void)argc;

  auto basic_info = *reinterpret_cast<const BasicInfo *>(argv);
  physical_memory_addr = basic_info.physical_memory_addr;
  physical_memory_size = basic_info.physical_memory_size;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);

  elf_addr = basic_info.elf_addr;
  elf_size = basic_info.elf_size;

  fdt_addr = 0;
}

auto main(uint32_t argc, uint8_t *argv) -> uint32_t {
  if (argc != 1) {
    printf("argc != 1 [%d]\n", argc);
    return -1;
  }

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  printf("Hello Test\n");

  return 0;
}

extern "C" void _start(uint32_t argc, uint8_t *argv) {
  main(argc, argv);

  // 进入死循环
  while (true) {
    ;
  }
}
