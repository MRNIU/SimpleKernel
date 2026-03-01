/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <opensbi_interface.h>

extern "C" void etl_putchar(int c) { sbi_debug_console_write_byte(c); }
