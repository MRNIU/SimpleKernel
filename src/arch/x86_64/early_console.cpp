/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>
#include <etl/singleton.h>

using SerialSingleton = etl::singleton<cpu_io::Serial>;

namespace {

cpu_io::Serial* serial = nullptr;

/// 早期控制台初始化结构体
struct EarlyConsole {
  EarlyConsole() {
    SerialSingleton::create(cpu_io::kCom1);
    serial = &SerialSingleton::instance();
  }
};

EarlyConsole early_console;

}  // namespace

extern "C" auto etl_putchar(int c) -> void {
  if (serial) {
    serial->Write(c);
  }
}
