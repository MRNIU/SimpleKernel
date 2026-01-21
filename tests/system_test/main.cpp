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
#include "syscall.hpp"
#include "system_test.h"

namespace {

struct test_case {
  const char* name;
  bool (*func)(void);
  // 是否为多核测试，需要所有核心参与
  bool is_smp_test = false;
};

std::array<test_case, 15> test_cases = {
    test_case{"ctor_dtor_test", ctor_dtor_test, false},
    test_case{"spinlock_test", spinlock_test, true},
    test_case{"memory_test", memory_test, false},
    test_case{"virtual_memory_test", virtual_memory_test, false},
    test_case{"interrupt_test", interrupt_test, false},
    test_case{"sk_list_test", sk_list_test, false},
    test_case{"sk_queue_test", sk_queue_test, false},
    test_case{"sk_vector_test", sk_vector_test, false},
    test_case{"sk_priority_queue_test", sk_priority_queue_test, false},
    test_case{"sk_rb_tree_test", sk_rb_tree_test, false},
    test_case{"sk_set_test", sk_set_test, false},
    test_case{"sk_unordered_map_test", sk_unordered_map_test, false},
    test_case{"fifo_scheduler_test", fifo_scheduler_test, false},
    test_case{"rr_scheduler_test", rr_scheduler_test, false},
    test_case{"cfs_scheduler_test", cfs_scheduler_test, false}};

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
  MemoryInitSMP();
  InterruptInitSMP(argc, argv);
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

  // 内存相关初始化
  MemoryInit();

  // 中断相关初始化
  InterruptInit(argc, argv);

  // 唤醒其余 core
  WakeUpOtherCores();

  DumpStack();

  klog::info << "Hello SimpleKernel\n";

  // 主核运行所有测试（包括多核测试）
  run_tests_main();

  return 0;
}
