
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
#include "sk_cstdio"

// printf_bare_metal 基本输出实现
extern "C" void _putchar(char character) {
  static auto* kUartAddr = (uint8_t*)0x09000000;
  *kUartAddr = character;
}

auto ArchInit(uint32_t argc, const uint8_t* argv) -> uint32_t {
  (void)argc;
  (void)argv;

  // 初始化 FPU
  cpu_io::SetupFpu();

  return 0;
}
