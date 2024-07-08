
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

#include <elf.h>

#include "basic_info.hpp"
#include "cpu.hpp"
#include "cstdio"
#include "cstring"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"

// printf_bare_metal 基本输出实现
/// @note 这里要注意，保证在 serial 初始化之前不能使用 printf
/// 函数，否则会有全局对象依赖问题
static cpu::Serial kSerial(cpu::kCom1);
extern "C" void _putchar(char character) { kSerial.Write(character); }

uint32_t ArchInit(uint32_t argc, uint8_t *argv) {
  if (argc != 1) {
    Err("argc != 1 [%d]\n", argc);
    throw;
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

  // 解析内核 elf 信息
  kKernelElf.GetInstance() = KernelElf(kBasicInfo.GetInstance().elf_addr,
                                       kBasicInfo.GetInstance().elf_size);

  Info("Hello x8_64 ArchInit\n");

  return 0;
}
