/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <opensbi_interface.h>

#include <cstdint>

#include "kstd_cstdio"

// printf_bare_metal 基本输出实现
extern "C" void putchar_(char character) {
  sbi_debug_console_write_byte(character);
}

uint32_t main(uint32_t, uint8_t*) {
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

extern "C" void _start(uint32_t argc, uint8_t* argv) {
  main(argc, argv);

  // 进入死循环
  while (1) {
    ;
  }
}
