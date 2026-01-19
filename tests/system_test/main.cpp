/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstdint>
#include <new>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"
#include "sk_libcxx.h"
#include "spinlock.hpp"
#include "system_test.h"

namespace {

struct test_case {
  const char* name;
  bool (*func)(void);
  // 是否为多核测试，需要所有核心参与
  bool is_smp_test;
};

std::array<test_case, 2> test_cases = {
    test_case{"ctor_dtor_test", ctor_dtor_test, false},
    test_case{"spinlock_test", spinlock_test, true},
};

/// 主核运行所有测试
void run_tests_main() {
  for (auto test : test_cases) {
    klog::Info("----%s----\n", test.name);
    if (test.func()) {
      klog::Info("----%s passed----\n", test.name);
    } else {
      klog::Err("----%s failed----\n", test.name);
    }
  }
  klog::Info("All tests done.\n");
}

/// 从核只参与多核测试
void run_tests_smp() {
  for (auto test : test_cases) {
    if (test.is_smp_test) {
      // 从核静默参与多核测试，不输出日志
      test.func();
    }
  }
}

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  ArchInitSMP(argc, argv);
  klog::Info("Hello SimpleKernel SMP\n");

  // 从核只参与多核测试
  run_tests_smp();

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

  // 主核运行所有测试（包括多核测试）
  run_tests_main();

  return 0;
}
