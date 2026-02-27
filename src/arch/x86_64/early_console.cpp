/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include "kstd_cstdio"
#include "singleton.hpp"

namespace {

cpu_io::Serial* serial = nullptr;

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (serial) {
    serial->Write(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    Singleton<cpu_io::Serial>::GetInstance() = cpu_io::Serial(cpu_io::kCom1);
    serial = &Singleton<cpu_io::Serial>::GetInstance();

    sk_putchar = console_putchar;
  }
};

EarlyConsole early_console;

}  // namespace
