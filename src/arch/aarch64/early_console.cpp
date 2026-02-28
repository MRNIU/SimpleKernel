/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <device_framework/pl011.hpp>

#include "kernel.h"
#include "kstd_cstdio"

namespace {

device_framework::pl011::Pl011Device* pl011 = nullptr;

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (pl011) {
    pl011->PutChar(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    Pl011Singleton::create(SIMPLEKERNEL_EARLY_CONSOLE_BASE);
    pl011 = &Pl011Singleton::instance();
    auto result = pl011->Open(
        device_framework::OpenFlags(device_framework::OpenFlags::kReadWrite));
    if (result) {
      sk_putchar = console_putchar;
    }
  }
};

EarlyConsole early_console;

}  // namespace
