/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <opensbi_interface.h>

#include "sk_cstdio"

namespace {

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  sbi_debug_console_write_byte(c);
}

struct EarlyConsole {
  EarlyConsole() { sk_putchar = console_putchar; }
};

EarlyConsole early_console;

}  // namespace
