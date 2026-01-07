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
#include "task.hpp"

// 声明 yield
void sys_yield();

void thread_func_a(void* arg) {
  while (1) {
    klog::Info("Thread A: running, arg=%d\n", (uint64_t)arg);
    // 模拟耗时操作
    for (volatile int i = 0; i < 1000000; i++)
      ;
    sys_yield();
  }
}

void thread_func_b(void* arg) {
  while (1) {
    klog::Info("Thread B: running, arg=%d\n", (uint64_t)arg);
    // 模拟耗时操作
    for (volatile int i = 0; i < 1000000; i++)
      ;
    sys_yield();
  }
}

namespace {

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  ArchInitSMP(argc, argv);
  MemoryInitSMP();
  InterruptInitSMP(argc, argv);
  klog::Info("Hello SimpleKernel SMP\n");
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

  // klog::Debug("Hello SimpleKernel\n");
  // klog::Info("Hello SimpleKernel\n");
  // klog::Warn("Hello SimpleKernel\n");
  // klog::Err("Hello SimpleKernel\n");

  DumpStack();

  TaskControlBlock("init", 1);

  klog::info << "Hello SimpleKernel\n";

  // 初始化任务管理器 (设置主线程)
  TaskManager::GetInstance().InitMainThread();

  // 创建线程 A
  // 注意：需要手动分配内存，实际中应由 ObjectPool 或 memory allocator
  // 管理生命周期
  TaskControlBlock* task_a = new TaskControlBlock("Task A", 1);
  task_a->InitThread(thread_func_a, (void*)100);
  TaskManager::GetInstance().AddTask(task_a);

  // 创建线程 B
  TaskControlBlock* task_b = new TaskControlBlock("Task B", 2);
  task_b->InitThread(thread_func_b, (void*)200);
  TaskManager::GetInstance().AddTask(task_b);

  klog::Info("Main: Starting scheduler...\n");

  // 主线程进入调度循环
  while (1) {
    klog::Info("Main Thread: running\n");
    for (volatile int i = 0; i < 2000000; i++)
      ;
    sys_yield();
  }

  return 0;
}
