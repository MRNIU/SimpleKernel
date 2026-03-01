/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kstd_cstdio"
#include "pl011/pl011_driver.hpp"
#include "pl011_singleton.h"

namespace {

pl011::Pl011Device* pl011_uart = nullptr;

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (pl011_uart) {
    pl011_uart->PutChar(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    Pl011Singleton::create(SIMPLEKERNEL_EARLY_CONSOLE_BASE);
    pl011_uart = &Pl011Singleton::instance();
    sk_putchar = console_putchar;  // Pl011 ctor already initializes HW
  }
};

EarlyConsole early_console;

}  // namespace

extern "C" void etl_putchar(int c) { sk_putchar(c, nullptr); }
