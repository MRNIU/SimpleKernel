/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "interrupt.h"

#include <cpu_io.h>

#include <cstddef>
#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt_base.h"
#include "kernel.h"
#include "singleton.hpp"
#include "sk_cstdio"
#include "sk_cstring"
#include "sk_libcxx.h"
#include "sk_stdlib.h"
#include "system_test.h"

auto interrupt_test() -> bool {
  sk_printf("memory_test: start\n");

  Singleton<Interrupt>::GetInstance().BroadcastIpi();

  sk_printf("memory_test: multi alloc passed\n");

  return true;
}
