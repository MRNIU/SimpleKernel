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
#include "task_control_block.hpp"
#include "task_manager.hpp"

namespace {

struct test_case {
  const char* name;
  bool (*func)(void);
};

std::array<test_case, 10> test_cases = {
    test_case{"ctor_dtor_test", ctor_dtor_test},
    test_case{"spinlock_test", spinlock_test},
    test_case{"memory_test", memory_test},
    test_case{"interrupt_test", interrupt_test},
    test_case{"sk_list_test", sk_list_test},
    test_case{"sk_queue_test", sk_queue_test},
    test_case{"sk_vector_test", sk_vector_test},
    test_case{"sk_priority_queue_test", sk_priority_queue_test},
    test_case{"sk_rb_tree_test", sk_rb_tree_test},
    test_case{"sk_set_test", sk_set_test},
    // test_case{"kernel_task_test", kernel_task_test},
    // test_case{"user_task_test", user_task_test},
};

void run_tests() {
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

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  ArchInitSMP(argc, argv);
  MemoryInitSMP();
  InterruptInitSMP(argc, argv);
  Singleton<TaskManager>::GetInstance().InitCurrentCore();
  klog::Info("Hello SimpleKernel SMP\n");

  run_tests();

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

  // 初始化任务管理器 (设置主线程)
  Singleton<TaskManager>::GetInstance().InitCurrentCore();

  // 唤醒其余 core
  WakeUpOtherCores();

  DumpStack();

  klog::info << "Hello SimpleKernel\n";

  run_tests();

  // 主线程进入调度循环
  while (1) {
    sys_sleep(100);
    sys_yield();
  }

  return 0;
}
