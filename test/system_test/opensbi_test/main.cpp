
/**
 * @file main.cpp
 * @brief 用于测试 opensbi 的启动
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

#include <opensbi_interface.h>

#include <cstdint>

#include "sk_cstdio"

// printf_bare_metal 基本输出实现
extern "C" void putchar_(char character) {
  sbi_debug_console_write_byte(character);
}

uint32_t main(uint32_t, uint8_t *) {
  putchar_('H');
  putchar_('e');
  putchar_('l');
  putchar_('l');
  putchar_('o');
  putchar_('W');
  putchar_('o');
  putchar_('r');
  putchar_('l');
  putchar_('d');
  putchar_('!');
  putchar_('\n');

  return 0;
}

extern "C" void _start(uint32_t argc, uint8_t *argv) {
  main(argc, argv);

  // 进入死循环
  while (1) {
    ;
  }
}
