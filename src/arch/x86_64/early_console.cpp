/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>
#include <etl/singleton.h>

#include "kstd_cstdio"

using SerialSingleton = etl::singleton<cpu_io::Serial>;

namespace {

cpu_io::Serial* serial = nullptr;

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (serial) {
    serial->Write(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    SerialSingleton::create(cpu_io::kCom1);
    serial = &SerialSingleton::instance();

    sk_putchar = console_putchar;
  }
};

EarlyConsole early_console;

}  // namespace
