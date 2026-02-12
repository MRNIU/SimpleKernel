/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "pl011.h"
#include "pl011_device.hpp"
#include "singleton.hpp"
#include "sk_cstdio"

namespace {

Pl011Device* pl011 = nullptr;

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (pl011) {
    pl011->PutChar(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    Singleton<Pl011Device>::GetInstance() =
        std::move(Pl011Device(SIMPLEKERNEL_EARLY_CONSOLE_BASE));
    pl011 = &Singleton<Pl011Device>::GetInstance();
    auto result = pl011->Open(OpenFlags(OpenFlags::kReadWrite));
    if (result) {
      sk_putchar = console_putchar;
    }
  }
};

EarlyConsole early_console;

}  // namespace
