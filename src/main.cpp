/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstdint>

#include "arch.h"
#include "arch/riscv64/interrupt.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_cstring"
#include "sk_iostream"
#include "sk_libcxx.h"
#include "sk_list"
#include "sk_new"
#include "sk_stdlib.h"
#include "sk_vector"
#include "syscall.hpp"
#include "task.hpp"
#include "virtual_memory.hpp"

namespace {

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  ArchInitSMP(argc, argv);
  MemoryInitSMP();
  InterruptInitSMP(argc, argv);
  Singleton<TaskManager>::GetInstance().InitCurrentCore();
  klog::Info("Hello SimpleKernel SMP\n");

  sys_yield();

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

void thread_func_a(void* arg) {
  while (1) {
    klog::Info("Thread A: running, arg=%d\n", (uint64_t)arg);
    sys_sleep(100);
  }
}

void thread_func_b(void* arg) {
  while (1) {
    klog::Info("Thread B: running, arg=%d\n", (uint64_t)arg);
    sys_sleep(100);
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

  auto task_a = new TaskControlBlock("Task A", 1, thread_func_a, (void*)100);
  auto task_b = new TaskControlBlock("Task B", 2, thread_func_b, (void*)200);
  Singleton<TaskManager>::GetInstance().AddTask(task_a);
  Singleton<TaskManager>::GetInstance().AddTask(task_b);

  klog::Info("Main: Starting scheduler...\n");

  // 主线程进入调度循环
  while (1) {
    klog::Info("Main Thread: running\n");
    sys_sleep(100);
    sys_yield();
  }

  return 0;
}
