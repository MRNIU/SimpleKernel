/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <device_framework/pl011.hpp>

#include "singleton.hpp"
#include "sk_cstdio"

namespace {

device_framework::pl011::Pl011Device* pl011 = nullptr;

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (pl011) {
    pl011->PutChar(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    Singleton<device_framework::pl011::Pl011Device>::GetInstance() = std::move(
        device_framework::pl011::Pl011Device(SIMPLEKERNEL_EARLY_CONSOLE_BASE));
    pl011 = &Singleton<device_framework::pl011::Pl011Device>::GetInstance();
    auto result = pl011->Open(
        device_framework::OpenFlags(device_framework::OpenFlags::kReadWrite));
    if (result) {
      sk_putchar = console_putchar;
    }
  }
};

EarlyConsole early_console;

}  // namespace
