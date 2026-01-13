/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "config.h"
#include "pl011.h"
#include "singleton.hpp"
#include "sk_cstdio"

namespace {

Pl011* pl011 = nullptr;

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (pl011) {
    pl011->PutChar(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    Singleton<Pl011>::GetInstance() = Pl011(SIMPLEKERNEL_EARLY_CONSOLE_BASE);
    pl011 = &Singleton<Pl011>::GetInstance();

    sk_putchar = console_putchar;
  }
};

EarlyConsole early_console;

}  // namespace
