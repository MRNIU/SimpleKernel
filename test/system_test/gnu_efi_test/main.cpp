
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

#include <cstdint>
#include <cstdio>
#include <cstring>

#include "basic_info.hpp"
#include "cpu.hpp"

extern "C" void _putchar(char character) {
  auto serial = cpu::Serial(cpu::kCom1);
  serial.Write(character);
}

uint32_t main(uint32_t argc, uint8_t *argv) {
  if (argc != 1) {
    printf("argc != 1 [%d]\n", argc);
    return -1;
  }

  kBasicInfo.GetInstance() = *reinterpret_cast<BasicInfo *>(argv);
  printf("kBasicInfo.elf_addr: 0x%X.\n", kBasicInfo.GetInstance().elf_addr);
  printf("kBasicInfo.elf_size: %d.\n", kBasicInfo.GetInstance().elf_size);

  for (uint32_t i = 0; i < kBasicInfo.GetInstance().memory_map_count; i++) {
    printf(
        "kBasicInfo.memory_map[%d].base_addr: 0x%p, length: 0x%X, type: %d.\n",
        i, kBasicInfo.GetInstance().memory_map[i].base_addr,
        kBasicInfo.GetInstance().memory_map[i].length,
        kBasicInfo.GetInstance().memory_map[i].type);
  }

  printf("Hello Test\n");

  return 0;
}

extern "C" void _start(uint32_t argc, uint8_t *argv) {
  main(argc, argv);

  // 进入死循环
  while (1) {
    ;
  }
}
