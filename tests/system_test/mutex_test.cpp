/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "mutex.hpp"

#include <atomic>
#include <cstdint>

#include "cpu_io.h"
#include "singleton.hpp"
#include "sk_stdio.h"
#include "spinlock.hpp"
#include "syscall.hpp"
#include "system_test.h"
#include "task_manager.hpp"

namespace {

// 测试用的共享变量
static std::atomic<int> shared_counter{0};
static std::atomic<int> finished_tasks{0};
static Mutex* test_mutex = nullptr;

/**
 * @brief 测试基本的 Lock/UnLock 功能
 */
auto test_basic_lock() -> bool {
  sk_printf("Running test_basic_lock...\n");

  Mutex mutex("basic_test");
  EXPECT_TRUE(mutex.Lock(), "Basic lock failed");
  EXPECT_TRUE(mutex.IsLockedByCurrentTask(),
              "IsLockedByCurrentTask failed after lock");
  EXPECT_TRUE(mutex.UnLock(), "Basic unlock failed");
  EXPECT_TRUE(!mutex.IsLockedByCurrentTask(),
              "IsLockedByCurrentTask failed after unlock");

  sk_printf("test_basic_lock passed\n");
  return true;
}

/**
 * @brief 测试递归加锁检测
 */
auto test_recursive_lock() -> bool {
  sk_printf("Running test_recursive_lock...\n");

  Mutex mutex("recursive_test");
  EXPECT_TRUE(mutex.Lock(), "Lock failed in recursive test");

  // 尝试递归获取锁应该失败
  if (mutex.Lock()) {
    sk_printf("FAIL: Recursive lock should return false\n");
    mutex.UnLock();  // 尝试恢复
    mutex.UnLock();
    return false;
  }

  EXPECT_TRUE(mutex.UnLock(), "Unlock failed in recursive test");

  // 再次解锁应该失败（未持有锁）
  if (mutex.UnLock()) {
    sk_printf("FAIL: Double unlock should return false\n");
    return false;
  }

  sk_printf("test_recursive_lock passed\n");
  return true;
}

/**
 * @brief 测试 TryLock 功能
 */
auto test_trylock() -> bool {
  sk_printf("Running test_trylock...\n");

  Mutex mutex("trylock_test");

  // 第一次 TryLock 应该成功
  EXPECT_TRUE(mutex.TryLock(), "First TryLock failed");
  EXPECT_TRUE(mutex.IsLockedByCurrentTask(), "TryLock didn't acquire lock");

  // 再次 TryLock 应该失败（递归）
  if (mutex.TryLock()) {
    sk_printf("FAIL: Recursive TryLock should return false\n");
    mutex.UnLock();
    return false;
  }

  EXPECT_TRUE(mutex.UnLock(), "UnLock after TryLock failed");

  sk_printf("test_trylock passed\n");
  return true;
}

/**
 * @brief 测试 LockGuard<Mutex> RAII
 */
auto test_mutex_guard() -> bool {
  sk_printf("Running test_mutex_guard...\n");

  Mutex mutex("guard_test");

  {
    LockGuard<Mutex> guard(mutex);
    EXPECT_TRUE(mutex.IsLockedByCurrentTask(), "LockGuard failed to lock");
  }
  // 作用域结束后应该自动解锁
  EXPECT_TRUE(!mutex.IsLockedByCurrentTask(), "LockGuard failed to unlock");

  sk_printf("test_mutex_guard passed\n");
  return true;
}

/**
 * @brief 测试多任务竞争互斥锁
 */
void mutex_contention_task(void* arg) {
  int task_id = *reinterpret_cast<int*>(arg);
  sk_printf("Task %d: started\n", task_id);

  for (int i = 0; i < 100; ++i) {
    // 获取互斥锁
    test_mutex->Lock();

    // 临界区：增加共享计数器
    int old_value = shared_counter.load();
    // 模拟一些计算
    for (volatile int j = 0; j < 10; ++j) {
      ;
    }
    shared_counter.store(old_value + 1);

    // 释放互斥锁
    test_mutex->UnLock();
  }

  sk_printf("Task %d: finished\n", task_id);
  finished_tasks.fetch_add(1);
}

/**
 * @brief 测试多任务竞争场景
 */
auto test_mutex_contention() -> bool {
  sk_printf("Running test_mutex_contention...\n");

  // 重置测试变量
  shared_counter = 0;
  finished_tasks = 0;

  // 创建测试用的互斥锁
  static Mutex mutex("contention_test");
  test_mutex = &mutex;

  // 创建多个任务竞争同一个锁
  constexpr int kNumTasks = 4;
  constexpr int kIterations = 100;
  static int task_ids[kNumTasks];

  auto& task_manager = Singleton<TaskManager>::GetInstance();

  for (int i = 0; i < kNumTasks; ++i) {
    task_ids[i] = i;
    auto* task = new TaskControlBlock("mutex_test_task", 10,
                                      mutex_contention_task, &task_ids[i]);
    task_manager.AddTask(task);
  }

  // 等待所有任务完成
  sk_printf("Waiting for tasks to complete...\n");
  while (finished_tasks.load() < kNumTasks) {
    // 让出 CPU，让其他任务运行
    sys_yield();
  }

  // 验证计数器值
  int expected = kNumTasks * kIterations;
  int actual = shared_counter.load();

  if (actual != expected) {
    sk_printf("FAIL: Expected counter=%d, got %d\n", expected, actual);
    return false;
  }

  sk_printf("test_mutex_contention passed (counter=%d)\n", actual);
  return true;
}

/**
 * @brief 测试 LockGuard<Mutex> 在多任务场景下的使用
 */
void mutex_guard_task(void* arg) {
  int task_id = *reinterpret_cast<int*>(arg);
  sk_printf("Task %d: started (with guard)\n", task_id);

  for (int i = 0; i < 100; ++i) {
    // 使用 RAII 风格的 LockGuard<Mutex>
    LockGuard<Mutex> guard(*test_mutex);

    // 临界区
    int old_value = shared_counter.load();
    for (volatile int j = 0; j < 10; ++j) {
      ;
    }
    shared_counter.store(old_value + 1);
    // guard 在作用域结束时自动释放锁
  }

  sk_printf("Task %d: finished (with guard)\n", task_id);
  finished_tasks.fetch_add(1);
}

/**
 * @brief 测试 LockGuard<Mutex> 在多任务竞争场景
 */
auto test_mutex_guard_contention() -> bool {
  sk_printf("Running test_mutex_guard_contention...\n");

  // 重置测试变量
  shared_counter = 0;
  finished_tasks = 0;

  static Mutex mutex("guard_contention_test");
  test_mutex = &mutex;

  constexpr int kNumTasks = 4;
  constexpr int kIterations = 100;
  static int task_ids[kNumTasks];

  auto& task_manager = Singleton<TaskManager>::GetInstance();

  for (int i = 0; i < kNumTasks; ++i) {
    task_ids[i] = i;
    auto* task = new TaskControlBlock("mutex_guard_test_task", 10,
                                      mutex_guard_task, &task_ids[i]);
    task_manager.AddTask(task);
  }

  // 等待所有任务完成
  sk_printf("Waiting for guard tasks to complete...\n");
  while (finished_tasks.load() < kNumTasks) {
    sys_yield();
  }

  // 验证计数器值
  int expected = kNumTasks * kIterations;
  int actual = shared_counter.load();

  if (actual != expected) {
    sk_printf("FAIL: Expected counter=%d, got %d\n", expected, actual);
    return false;
  }

  sk_printf("test_mutex_guard_contention passed (counter=%d)\n", actual);
  return true;
}

}  // namespace

namespace MutexTest {

void RunTest() {
  sk_printf("\n========== Mutex System Tests ==========\n");

  bool all_passed = true;

  all_passed &= test_basic_lock();
  all_passed &= test_recursive_lock();
  all_passed &= test_trylock();
  all_passed &= test_mutex_guard();
  all_passed &= test_mutex_contention();
  all_passed &= test_mutex_guard_contention();

  if (all_passed) {
    sk_printf("\n[PASS] All Mutex tests passed!\n");
  } else {
    sk_printf("\n[FAIL] Some Mutex tests failed!\n");
  }

  sk_printf("========== Mutex Tests Complete ==========\n\n");
}

}  // namespace MutexTest
