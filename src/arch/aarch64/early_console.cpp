/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "pl011/pl011_driver.hpp"
#include "pl011_singleton.h"

namespace {

pl011::Pl011Device* pl011_uart = nullptr;

struct EarlyConsole {
  EarlyConsole() {
    Pl011Singleton::create(SIMPLEKERNEL_EARLY_CONSOLE_BASE);
    pl011_uart = &Pl011Singleton::instance();
  }
};

EarlyConsole early_console;

}  // namespace

extern "C" void etl_putchar(int c) {
  if (pl011_uart) {
    pl011_uart->PutChar(c);
  }
}
