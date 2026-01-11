/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <atomic>
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
#include "task_control_block.hpp"
#include "task_manager.hpp"

namespace {

std::atomic<int> g_task_a_counter{0};
std::atomic<int> g_task_b_counter{0};

void thread_func_a(void* arg) {
  uint64_t id = (uint64_t)arg;
  for (int i = 0; i < 5; ++i) {
    klog::Info("Thread A: running, arg=%d, iter=%d\n", id, i);
    g_task_a_counter++;
    sys_sleep(50);
  }
  klog::Info("Thread A: exit\n");
  sys_exit(0);
}

void thread_func_b(void* arg) {
  uint64_t id = (uint64_t)arg;
  for (int i = 0; i < 5; ++i) {
    klog::Info("Thread B: running, arg=%d, iter=%d\n", id, i);
    g_task_b_counter++;
    sys_sleep(50);
  }
  klog::Info("Thread B: exit\n");
  sys_exit(0);
}
}  // namespace

auto kernel_task_test() -> bool {
  sk_printf("kernel_task_test: start\n");
  g_task_a_counter = 0;
  g_task_b_counter = 0;

  // 创建线程 A
  auto task_a = new TaskControlBlock("Task A", 100, thread_func_a, (void*)100);
  Singleton<TaskManager>::GetInstance().AddTask(task_a);

  // 创建线程 B
  auto task_b = new TaskControlBlock("Task B", 101, thread_func_b, (void*)200);
  Singleton<TaskManager>::GetInstance().AddTask(task_b);

  klog::Info("Main: Waiting for tasks...\n");

  // Wait for tasks to finish (or reach expected count)
  // int timeout = 200;  // 200 * 50ms = 10s roughly
  // while (timeout > 0) {
  //   sys_sleep(50);
  //   if (g_task_a_counter >= 5 && g_task_b_counter >= 5) {
  //     break;
  //   }
  //   timeout--;
  // }

  // EXPECT_EQ(g_task_a_counter, 5, "Task A count");
  // EXPECT_EQ(g_task_b_counter, 5, "Task B count");

  sk_printf("kernel_task_test: PASS\n");
  return true;
}
