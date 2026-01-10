/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <cstddef>
#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "sk_cstdio"
#include "sk_cstring"
#include "sk_libcxx.h"
#include "sk_stdlib.h"
#include "syscall.hpp"
#include "system_test.h"
#include "task.hpp"

namespace {

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
}  // namespace

auto kernel_task_test() -> bool {
  sk_printf("kernel_task_test: start\n");

  // 初始化任务管理器 (设置主线程)
  Singleton<TaskManager>::GetInstance().InitMainThread();

  // 创建线程 A
  // 注意：需要手动分配内存，实际中应由 ObjectPool 或 memory allocator
  // 管理生命周期
  auto task_a = new TaskControlBlock("Task A", 1, thread_func_a, (void*)100);
  Singleton<TaskManager>::GetInstance().AddTask(task_a);

  // 创建线程 B
  auto task_b = new TaskControlBlock("Task B", 2, thread_func_b, (void*)200);
  Singleton<TaskManager>::GetInstance().AddTask(task_b);

  klog::Info("Main: Starting scheduler...\n");

  // 主线程进入调度循环
  while (1) {
    klog::Info("Main Thread: running\n");
    sys_sleep(100);
    sys_yield();
  }

  return true;
}
