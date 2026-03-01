/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>
#include <etl/singleton.h>

using SerialSingleton = etl::singleton<cpu_io::Serial>;

namespace {

cpu_io::Serial* serial = nullptr;

struct EarlyConsole {
  EarlyConsole() {
    SerialSingleton::create(cpu_io::kCom1);
    serial = &SerialSingleton::instance();
  }
};

EarlyConsole early_console;

}  // namespace

extern "C" void etl_putchar(int c) {
  if (serial) {
    serial->Write(c);
  }
}
