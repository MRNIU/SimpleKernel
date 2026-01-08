/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"
#include "sk_libcxx.h"
#include "sk_new"
#include "system_test.h"

namespace {

void tests() {
  klog::Info("----ctor_dtor_test----\n");
  ctor_dtor_test();
  klog::Info("----ctor_dtor_test finished----\n");
}

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  ArchInitSMP(argc, argv);
  klog::Info("Hello SimpleKernel SMP\n");

  tests();

  return 0;
}

}  // namespace

void _start(int argc, const char** argv) {
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

auto main(int argc, const char** argv) -> int {
  // 架构相关初始化
  ArchInit(argc, argv);

  // 唤醒其余 core
  WakeUpOtherCores();

  DumpStack();

  klog::info << "Hello SimpleKernel\n";

  tests();

  return 0;
}
