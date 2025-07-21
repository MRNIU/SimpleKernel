/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 内核入口
 */

#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"
#include "sk_libcxx.h"

namespace {

/// 非启动核入口
auto main_smp(int argc, const char **argv) -> int {
  ArchInitSMP(argc, argv);
  InterruptInitSMP(argc, argv);
  klog::Info("Hello SimpleKernel SMP\n");
  return 0;
}

}  // namespace

void _start(int argc, const char **argv) {
  if (argv != nullptr) {
    CppInit();
    main(argc, argv);
    CppDeInit();
  } else {
    main_smp(argc, argv);
  }

  // 进入死循环
  while (true) {
    ;
  }
}

auto main(int argc, const char **argv) -> int {
  ArchInit(argc, argv);
  InterruptInit(argc, argv);

  DumpStack();

  klog::info << "Hello SimpleKernel\n";

  return 0;
}
