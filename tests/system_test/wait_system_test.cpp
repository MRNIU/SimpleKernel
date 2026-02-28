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
#include "kstd_cstdio"
#include "kstd_cstring"
#include "kstd_libcxx.h"
#include "sk_stdlib.h"
#include "syscall.hpp"
#include "system_test.h"
#include "task_control_block.hpp"
#include "task_manager.hpp"

namespace {

std::atomic<int> g_child_exit_code{0};
std::atomic<int> g_wait_completed{0};

std::atomic<int> g_tests_completed{0};
std::atomic<int> g_tests_failed{0};
/**
 * @brief 子进程工作函数
 */
void child_work(void* arg) {
  uint64_t child_id = reinterpret_cast<uint64_t>(arg);
  int exit_code = static_cast<int>(child_id);

  klog::Info("Child %zu: starting, will exit with code %d\n", child_id,
             exit_code);

  // 执行一些工作
  for (int i = 0; i < 5; ++i) {
    klog::Debug("Child %zu: working, iter=%d\n", child_id, i);
    sys_sleep(30);
  }

  klog::Info("Child %zu: exiting with code %d\n", child_id, exit_code);
  g_child_exit_code = exit_code;
  sys_exit(exit_code);
}

/**
 * @brief 测试基本的 wait 功能
 */
void test_wait_basic(void* /*arg*/) {
  klog::Info("=== Wait Basic Test ===\n");

  auto& task_mgr = TaskManagerSingleton::instance();
  auto* current = task_mgr.GetCurrentTask();

  if (!current) {
    klog::Err("Wait Basic Test: Cannot get current task\n");
    sys_exit(1);
  }

  // 创建子进程
  auto* child = new TaskControlBlock("WaitChild", 10, child_work,
                                     reinterpret_cast<void*>(42));
  child->parent_pid = current->pid;
  child->pgid = current->pgid;

  task_mgr.AddTask(child);
  Pid child_pid = child->pid;

  klog::Info("Parent: created child with PID=%zu\n", child_pid);

  // 等待子进程退出
  int status = 0;
  Pid result = task_mgr.Wait(child_pid, &status, false, false).value_or(0);

  if (result == child_pid) {
    klog::Info("Parent: child %zu exited with status %d\n", result, status);
    if (status == 42) {
      klog::Info("Wait Basic Test: PASS\n");
    } else {
      klog::Err(
          "Wait Basic Test: FAIL - wrong exit status (got %d, expected "
          "42)\n",
          status);
    }
  } else {
    klog::Err("Wait Basic Test: FAIL - wait returned %zu (expected %zu)\n",
              result, child_pid);
  }

  if (result == child_pid && status == 42) {
    klog::Info("Wait Basic Test: PASS\n");
  } else {
    klog::Err("Wait Basic Test: FAIL\n");
    g_tests_failed++;
  }

  g_tests_completed++;
  sys_exit(0);
}

/**
 * @brief 测试等待任意子进程
 */
void test_wait_any_child(void* /*arg*/) {
  klog::Info("=== Wait Any Child Test ===\n");

  auto& task_mgr = TaskManagerSingleton::instance();
  auto* current = task_mgr.GetCurrentTask();

  if (!current) {
    klog::Err("Wait Any Child Test: Cannot get current task\n");
    sys_exit(1);
  }

  // 创建多个子进程
  constexpr int kChildCount = 3;
  for (int i = 0; i < kChildCount; ++i) {
    auto* child = new TaskControlBlock("AnyChild", 10, child_work,
                                       reinterpret_cast<void*>(10 + i));
    child->parent_pid = current->pid;
    child->pgid = current->pgid;
    task_mgr.AddTask(child);
    klog::Info("Parent: created child %d with PID=%zu\n", i, child->pid);
  }

  // 等待任意子进程 (pid = -1)
  int completed = 0;
  for (int i = 0; i < kChildCount; ++i) {
    int status = 0;
    Pid result =
        task_mgr.Wait(static_cast<Pid>(-1), &status, false, false).value_or(0);

    if (result > 0) {
      klog::Info("Parent: child PID=%zu exited with status %d\n", result,
                 status);
      completed++;
    } else {
      klog::Err("Parent: wait failed with result %zu\n", result);
      break;
    }
  }

  if (completed == kChildCount) {
    klog::Info("Wait Any Child Test: PASS\n");
  } else {
    klog::Err("Wait Any Child Test: FAIL - only %d/%d children reaped\n",
              completed, kChildCount);
  }

  g_tests_completed++;
  sys_exit(0);
}

/**
 * @brief 测试非阻塞 wait (WNOHANG)
 */
void slow_child_work(void* arg) {
  uint64_t child_id = reinterpret_cast<uint64_t>(arg);

  klog::Info("SlowChild %zu: starting\n", child_id);

  // 执行较长时间的工作
  for (int i = 0; i < 10; ++i) {
    klog::Debug("SlowChild %zu: working, iter=%d\n", child_id, i);
    sys_sleep(100);
  }

  klog::Info("SlowChild %zu: exiting\n", child_id);
  sys_exit(0);
}

void test_wait_no_hang(void* /*arg*/) {
  klog::Info("=== Wait NoHang Test ===\n");

  auto& task_mgr = TaskManagerSingleton::instance();
  auto* current = task_mgr.GetCurrentTask();

  if (!current) {
    klog::Err("Wait NoHang Test: Cannot get current task\n");
    sys_exit(1);
  }

  // 创建一个慢速子进程
  auto* child = new TaskControlBlock("SlowChild", 10, slow_child_work,
                                     reinterpret_cast<void*>(1));
  child->parent_pid = current->pid;
  child->pgid = current->pgid;
  task_mgr.AddTask(child);
  Pid child_pid = child->pid;

  klog::Info("Parent: created slow child with PID=%zu\n", child_pid);

  // 立即尝试非阻塞 wait
  int status = 0;
  Pid result = task_mgr.Wait(child_pid, &status, true, false).value_or(0);

  if (result == 0) {
    klog::Info("Parent: no-hang wait returned 0 (child still running)\n");
    klog::Info("Wait NoHang Test: PASS\n");
  } else {
    klog::Err(
        "Wait NoHang Test: FAIL - expected 0, got %zu (child shouldn't have "
        "exited)\n",
        result);
  }

  // 清理：等待子进程实际退出（阻塞）
  result = task_mgr.Wait(child_pid, &status, false, false).value_or(0);
  klog::Info("Parent: child finally exited with PID=%zu\n", result);

  g_tests_completed++;
  sys_exit(0);
}

/**
 * @brief 测试等待同进程组的子进程
 */
void test_wait_process_group(void* /*arg*/) {
  klog::Info("=== Wait Process Group Test ===\n");

  auto& task_mgr = TaskManagerSingleton::instance();
  auto* current = task_mgr.GetCurrentTask();

  if (!current) {
    klog::Err("Wait Process Group Test: Cannot get current task\n");
    sys_exit(1);
  }

  // 创建同进程组的子进程
  auto* child1 = new TaskControlBlock("PgChild1", 10, child_work,
                                      reinterpret_cast<void*>(1));
  child1->parent_pid = current->pid;
  child1->pgid = current->pgid;  // 同一进程组
  task_mgr.AddTask(child1);

  auto* child2 = new TaskControlBlock("PgChild2", 10, child_work,
                                      reinterpret_cast<void*>(2));
  child2->parent_pid = current->pid;
  child2->pgid = 9999;  // 不同进程组
  task_mgr.AddTask(child2);

  klog::Info("Parent: created child1 (pgid=%zu) and child2 (pgid=%zu)\n",
             child1->pgid, child2->pgid);

  // 等待同进程组的子进程 (pid = 0)
  int status = 0;
  Pid result = task_mgr.Wait(0, &status, false, false).value_or(0);

  if (result == child1->pid) {
    klog::Info("Parent: correctly waited for same-pgid child %zu\n", result);
    klog::Info("Wait Process Group Test: PASS\n");
  } else {
    klog::Err("Wait Process Group Test: FAIL - got PID=%zu, expected %zu\n",
              result, child1->pid);
  }

  // 清理另一个子进程
  result = task_mgr.Wait(child2->pid, &status, false, false).value_or(0);
  klog::Info("Parent: cleaned up child2 PID=%zu\n", result);

  g_tests_completed++;
  sys_exit(0);
}

/**
 * @brief 测试僵尸进程回收
 */
void zombie_child_work(void* arg) {
  uint64_t child_id = reinterpret_cast<uint64_t>(arg);
  klog::Info("ZombieChild %zu: exiting immediately\n", child_id);
  g_wait_completed++;
  sys_exit(0);
}

void test_wait_zombie_reap(void* /*arg*/) {
  klog::Info("=== Wait Zombie Reap Test ===\n");

  g_wait_completed = 0;
  auto& task_mgr = TaskManagerSingleton::instance();
  auto* current = task_mgr.GetCurrentTask();

  if (!current) {
    klog::Err("Wait Zombie Reap Test: Cannot get current task\n");
    sys_exit(1);
  }

  // 创建快速退出的子进程（会变成僵尸）
  auto* child = new TaskControlBlock("ZombieChild", 10, zombie_child_work,
                                     reinterpret_cast<void*>(1));
  child->parent_pid = current->pid;
  child->pgid = current->pgid;
  task_mgr.AddTask(child);
  Pid child_pid = child->pid;

  klog::Info("Parent: created zombie child with PID=%zu\n", child_pid);

  // 等待一段时间让子进程变成僵尸
  sys_sleep(200);

  // 回收僵尸进程
  int status = 0;
  Pid result = task_mgr.Wait(child_pid, &status, false, false).value_or(0);

  if (result == child_pid) {
    klog::Info("Parent: successfully reaped zombie child %zu\n", result);
    klog::Info("Wait Zombie Reap Test: PASS\n");
  } else {
    klog::Err("Wait Zombie Reap Test: FAIL - wait returned %zu\n", result);
  }

  g_tests_completed++;
  sys_exit(0);
}

}  // namespace

/**
 * @brief Wait 系统测试入口
 */
auto wait_system_test() -> bool {
  sk_printf("=== Wait System Test Suite ===\n");

  g_tests_completed = 0;
  g_tests_failed = 0;

  auto& task_mgr = TaskManagerSingleton::instance();

  // 测试 1: 基本 wait 功能
  auto* test1 =
      new TaskControlBlock("TestWaitBasic", 10, test_wait_basic, nullptr);
  task_mgr.AddTask(test1);

  // 测试 2: 等待任意子进程
  auto* test2 = new TaskControlBlock("TestWaitAnyChild", 10,
                                     test_wait_any_child, nullptr);
  task_mgr.AddTask(test2);

  // 测试 3: 非阻塞 wait
  auto* test3 =
      new TaskControlBlock("TestWaitNoHang", 10, test_wait_no_hang, nullptr);
  task_mgr.AddTask(test3);

  // 测试 4: 进程组 wait
  auto* test4 = new TaskControlBlock("TestWaitProcessGroup", 10,
                                     test_wait_process_group, nullptr);
  task_mgr.AddTask(test4);

  // 测试 5: 僵尸进程回收
  auto* test5 = new TaskControlBlock("TestWaitZombieReap", 10,
                                     test_wait_zombie_reap, nullptr);
  task_mgr.AddTask(test5);

  // 同步等待所有测试完成
  constexpr int kExpectedTests = 5;
  int timeout = 400;
  while (timeout > 0) {
    sys_sleep(50);
    if (g_tests_completed >= kExpectedTests) {
      break;
    }
    timeout--;
  }

  EXPECT_EQ(g_tests_completed.load(), kExpectedTests,
            "All wait tests should complete");
  EXPECT_EQ(g_tests_failed.load(), 0, "No wait tests should fail");

  sk_printf("Wait System Test Suite: COMPLETED\n");
  return true;
}
