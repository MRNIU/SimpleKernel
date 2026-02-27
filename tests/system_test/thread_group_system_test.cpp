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

std::atomic<int> g_thread_counter{0};
std::atomic<int> g_thread_completed{0};

std::atomic<int> g_tests_completed{0};
std::atomic<int> g_tests_failed{0};
/**
 * @brief 线程函数，增加计数器
 */
void thread_increment(void* arg) {
  uint64_t thread_id = reinterpret_cast<uint64_t>(arg);

  for (int i = 0; i < 10; ++i) {
    g_thread_counter++;
    klog::Debug("Thread %zu: counter=%d, iter=%d\n", thread_id,
                g_thread_counter.load(), i);
    sys_sleep(10);
  }

  g_thread_completed++;
  klog::Info("Thread %zu: completed\n", thread_id);
  sys_exit(0);
}

/**
 * @brief 测试线程组的基本功能
 */
void test_thread_group_basic(void* /*arg*/) {
  klog::Info("=== Thread Group Basic Test ===\n");

  g_thread_counter = 0;
  g_thread_completed = 0;

  // 创建主线程
  auto* leader =
      new TaskControlBlock("ThreadGroupLeader", 10, nullptr, nullptr);
  leader->pid = 1000;
  leader->tgid = 1000;

  // 创建并加入线程组的线程
  auto* thread1 = new TaskControlBlock("Thread1", 10, thread_increment,
                                       reinterpret_cast<void*>(1));
  thread1->pid = 1001;
  thread1->JoinThreadGroup(leader);

  auto* thread2 = new TaskControlBlock("Thread2", 10, thread_increment,
                                       reinterpret_cast<void*>(2));
  thread2->pid = 1002;
  thread2->JoinThreadGroup(leader);

  auto* thread3 = new TaskControlBlock("Thread3", 10, thread_increment,
                                       reinterpret_cast<void*>(3));
  thread3->pid = 1003;
  thread3->JoinThreadGroup(leader);

  // 验证线程组大小
  size_t group_size = leader->GetThreadGroupSize();
  klog::Info("Thread group size: %zu (expected 4)\n", group_size);

  // 验证所有线程在同一线程组
  if (leader->InSameThreadGroup(thread1) &&
      leader->InSameThreadGroup(thread2) &&
      leader->InSameThreadGroup(thread3)) {
    klog::Info("All threads are in the same thread group: PASS\n");
  } else {
    klog::Err("Thread group membership check failed: FAIL\n");
  }

  // 添加到调度器
  auto& task_mgr = Singleton<TaskManager>::GetInstance();
  task_mgr.AddTask(thread1);
  task_mgr.AddTask(thread2);
  task_mgr.AddTask(thread3);

  // 等待线程完成
  for (int i = 0; i < 200 && g_thread_completed < 3; ++i) {
    sys_sleep(50);
  }

  klog::Info("Thread completed count: %d (expected 3)\n",
             g_thread_completed.load());
  klog::Info("Final counter value: %d (expected 30)\n",
             g_thread_counter.load());

  // 清理
  delete leader;

  bool passed = (g_thread_completed == 3 && g_thread_counter >= 30);
  if (passed) {
    klog::Info("Thread Group Basic Test: PASS\n");
  } else {
    klog::Err("Thread Group Basic Test: FAIL\n");
    g_tests_failed++;
  }

  g_tests_completed++;
  sys_exit(0);
}

/**
 * @brief 测试线程组的动态加入和离开
 */
void test_thread_group_dynamic(void* /*arg*/) {
  klog::Info("=== Thread Group Dynamic Test ===\n");

  // 创建主线程
  auto* leader = new TaskControlBlock("DynamicLeader", 10, nullptr, nullptr);
  leader->pid = 2000;
  leader->tgid = 2000;

  // 创建线程池
  constexpr int kThreadCount = 5;
  TaskControlBlock* threads[kThreadCount];

  for (int i = 0; i < kThreadCount; ++i) {
    threads[i] = new TaskControlBlock("DynamicThread", 10, nullptr, nullptr);
    threads[i]->pid = 2001 + i;
  }

  // 动态加入
  klog::Info("Joining threads...\n");
  for (int i = 0; i < kThreadCount; ++i) {
    threads[i]->JoinThreadGroup(leader);
    size_t size = leader->GetThreadGroupSize();
    klog::Debug("After join %d: group size=%zu\n", i, size);
  }

  size_t final_size = leader->GetThreadGroupSize();
  klog::Info("Final group size: %zu (expected %d)\n", final_size,
             kThreadCount + 1);

  // 动态离开
  klog::Info("Leaving threads...\n");
  for (int i = 0; i < kThreadCount; ++i) {
    threads[i]->LeaveThreadGroup();
    size_t size = leader->GetThreadGroupSize();
    klog::Debug("After leave %d: group size=%zu\n", i, size);
  }

  size_t remaining_size = leader->GetThreadGroupSize();
  klog::Info("Remaining group size: %zu (expected 1)\n", remaining_size);

  // 清理
  for (int i = 0; i < kThreadCount; ++i) {
    delete threads[i];
  }
  delete leader;

  bool passed = (final_size == static_cast<size_t>(kThreadCount + 1) &&
                 remaining_size == 1);
  if (passed) {
    klog::Info("Thread Group Dynamic Test: PASS\n");
  } else {
    klog::Err("Thread Group Dynamic Test: FAIL\n");
    g_tests_failed++;
  }

  g_tests_completed++;
  sys_exit(0);
}

/**
 * @brief 测试线程组成员同时退出
 */
void concurrent_exit_worker(void* arg) {
  uint64_t thread_id = reinterpret_cast<uint64_t>(arg);

  // 执行一些工作
  for (int i = 0; i < 5; ++i) {
    klog::Debug("ConcurrentExitWorker %zu: iter=%d\n", thread_id, i);
    sys_sleep(20);
  }

  klog::Info("ConcurrentExitWorker %zu: exiting\n", thread_id);
  g_thread_completed++;
  sys_exit(0);
}

void test_thread_group_concurrent_exit(void* /*arg*/) {
  klog::Info("=== Thread Group Concurrent Exit Test ===\n");

  g_thread_completed = 0;

  // 创建主线程
  auto* leader = new TaskControlBlock("ConcurrentLeader", 10, nullptr, nullptr);
  leader->pid = 3000;
  leader->tgid = 3000;

  // 创建多个工作线程
  constexpr int kWorkerCount = 4;
  for (int i = 0; i < kWorkerCount; ++i) {
    auto* worker =
        new TaskControlBlock("ConcurrentWorker", 10, concurrent_exit_worker,
                             reinterpret_cast<void*>(i));
    worker->pid = 3001 + i;
    worker->JoinThreadGroup(leader);
    Singleton<TaskManager>::GetInstance().AddTask(worker);
  }

  klog::Info("Started %d worker threads\n", kWorkerCount);

  // 等待所有线程完成
  for (int i = 0; i < 100 && g_thread_completed < kWorkerCount; ++i) {
    sys_sleep(50);
  }

  klog::Info("Completed threads: %d (expected %d)\n", g_thread_completed.load(),
             kWorkerCount);

  delete leader;

  bool passed = (g_thread_completed == kWorkerCount);
  if (passed) {
    klog::Info("Thread Group Concurrent Exit Test: PASS\n");
  } else {
    klog::Err("Thread Group Concurrent Exit Test: FAIL\n");
    g_tests_failed++;
  }

  g_tests_completed++;
  sys_exit(0);
}

}  // namespace

/**
 * @brief 线程组系统测试入口
 */
auto thread_group_system_test() -> bool {
  sk_printf("=== Thread Group System Test Suite ===\n");

  g_tests_completed = 0;
  g_tests_failed = 0;

  // 测试 1: 基本线程组功能
  auto* test1 = new TaskControlBlock("TestThreadGroupBasic", 10,
                                     test_thread_group_basic, nullptr);
  Singleton<TaskManager>::GetInstance().AddTask(test1);

  // 测试 2: 动态加入和离开
  auto* test2 = new TaskControlBlock("TestThreadGroupDynamic", 10,
                                     test_thread_group_dynamic, nullptr);
  Singleton<TaskManager>::GetInstance().AddTask(test2);

  // 测试 3: 并发退出
  auto* test3 =
      new TaskControlBlock("TestThreadGroupConcurrentExit", 10,
                           test_thread_group_concurrent_exit, nullptr);
  Singleton<TaskManager>::GetInstance().AddTask(test3);

  // 同步等待所有测试完成
  constexpr int kExpectedTests = 3;
  int timeout = 400;
  while (timeout > 0) {
    sys_sleep(50);
    if (g_tests_completed >= kExpectedTests) {
      break;
    }
    timeout--;
  }

  EXPECT_EQ(g_tests_completed.load(), kExpectedTests,
            "All thread group tests should complete");
  EXPECT_EQ(g_tests_failed.load(), 0, "No thread group tests should fail");

  sk_printf("Thread Group System Test Suite: COMPLETED\n");
  return true;
}
