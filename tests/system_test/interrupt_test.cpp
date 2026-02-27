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
#include "kstd_cstdio"
#include "kstd_cstring"
#include "kstd_libcxx.h"
#include "singleton.hpp"
#include "sk_stdlib.h"
#include "system_test.h"

/// @todo 等用户态调通后补上
auto interrupt_test() -> bool {
  sk_printf("interrupt_test: start\n");

  Singleton<Interrupt>::GetInstance().BroadcastIpi();

  sk_printf("interrupt_test: broadcast ipi passed\n");

  return true;
}
