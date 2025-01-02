
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

#include "arch.h"
#include "cpu/cpu.hpp"
#include "sk_cstdio"

// printf_bare_metal 基本输出实现
static uint8_t* kUartAddr = (uint8_t*)0x09000000;
extern "C" void _putchar(char character) { *kUartAddr = character; }

uint32_t ArchInit(uint32_t argc, uint8_t* argv) {
  (void)argc;
  (void)argv;

  // 初始化 FPU
  cpu::SetupFpu();

  return 0;
}
