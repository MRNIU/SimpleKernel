/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <opensbi_interface.h>

extern "C" auto etl_putchar(int c) -> void { sbi_debug_console_write_byte(c); }
