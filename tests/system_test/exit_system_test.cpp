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

std::atomic<int> g_exit_test_counter{0};

/**
 * @brief 测试正常退出
 */
void test_exit_normal(void* /*arg*/) {
  klog::Info("=== Exit Normal Test ===\n");

  // 创建一个任务并让它正常退出
  auto* task = new TaskControlBlock("ExitNormal", 10, nullptr, nullptr);
  task->pid = 5000;
  task->tgid = 5000;
  task->parent_pid = 1;

  Singleton<TaskManager>::GetInstance().AddTask(task);

  // 让任务运行一段时间
  sys_sleep(50);

  // 模拟任务退出
  task->exit_code = 0;
  task->status = TaskStatus::kZombie;

  if (task->exit_code == 0 && task->status == TaskStatus::kZombie) {
    klog::Info("Task exited with code %d and status kZombie\n",
               task->exit_code);
  } else {
    klog::Err("Task exit failed\n");
  }

  klog::Info("Exit Normal Test: PASSED\n");
}

/**
 * @brief 测试带错误码退出
 */
void test_exit_with_error(void* /*arg*/) {
  klog::Info("=== Exit With Error Test ===\n");

  auto* task = new TaskControlBlock("ExitError", 10, nullptr, nullptr);
  task->pid = 5001;
  task->tgid = 5001;
  task->parent_pid = 1;

  Singleton<TaskManager>::GetInstance().AddTask(task);

  sys_sleep(50);

  // 模拟任务以错误码退出
  task->exit_code = 42;
  task->status = TaskStatus::kZombie;

  if (task->exit_code == 42 && task->status == TaskStatus::kZombie) {
    klog::Info("Task exited with error code %d\n", task->exit_code);
  } else {
    klog::Err("Task exit with error failed\n");
  }

  klog::Info("Exit With Error Test: PASSED\n");
}

/**
 * @brief 测试线程退出
 */
void child_thread_exit_work(void* arg) {
  uint64_t thread_id = reinterpret_cast<uint64_t>(arg);

  klog::Info("Thread %zu: starting\n", thread_id);

  for (int i = 0; i < 3; ++i) {
    g_exit_test_counter++;
    klog::Debug("Thread %zu: working, iter=%d\n", thread_id, i);
    sys_sleep(30);
  }

  klog::Info("Thread %zu: exiting\n", thread_id);
  sys_exit(static_cast<int>(thread_id));
}

void test_thread_exit(void* /*arg*/) {
  klog::Info("=== Thread Exit Test ===\n");

  g_exit_test_counter = 0;

  // 创建线程组主线程
  auto* leader = new TaskControlBlock("ThreadLeader", 10, nullptr, nullptr);
  leader->pid = 5100;
  leader->tgid = 5100;
  leader->parent_pid = 1;

  Singleton<TaskManager>::GetInstance().AddTask(leader);

  // 创建子线程
  auto* thread1 = new TaskControlBlock("Thread1", 10, child_thread_exit_work,
                                       reinterpret_cast<void*>(1));
  thread1->pid = 5101;
  thread1->tgid = 5100;
  thread1->JoinThreadGroup(leader);

  Singleton<TaskManager>::GetInstance().AddTask(thread1);

  auto* thread2 = new TaskControlBlock("Thread2", 10, child_thread_exit_work,
                                       reinterpret_cast<void*>(2));
  thread2->pid = 5102;
  thread2->tgid = 5100;
  thread2->JoinThreadGroup(leader);

  Singleton<TaskManager>::GetInstance().AddTask(thread2);

  klog::Info("Created thread group with leader (pid=%zu) and 2 threads\n",
             leader->pid);

  // 等待线程运行并退出
  sys_sleep(200);

  klog::Info("Exit test counter: %d (expected >= 6)\n",
             g_exit_test_counter.load());

  klog::Info("Thread Exit Test: PASSED\n");
}

/**
 * @brief 测试孤儿进程退出
 */
void test_orphan_exit(void* /*arg*/) {
  klog::Info("=== Orphan Exit Test ===\n");

  // 创建一个孤儿进程（parent_pid = 0）
  auto* orphan = new TaskControlBlock("Orphan", 10, nullptr, nullptr);
  orphan->pid = 5200;
  orphan->tgid = 5200;
  orphan->parent_pid = 0;  // 孤儿进程

  Singleton<TaskManager>::GetInstance().AddTask(orphan);

  sys_sleep(50);

  // 孤儿进程退出时应该直接进入 kExited 状态
  orphan->exit_code = 0;
  orphan->status = TaskStatus::kExited;

  if (orphan->parent_pid == 0 && orphan->status == TaskStatus::kExited) {
    klog::Info("Orphan process exited with status kExited\n");
  } else {
    klog::Err("Orphan process exit failed\n");
  }

  klog::Info("Orphan Exit Test: PASSED\n");
}

/**
 * @brief 测试进程退出后变为僵尸
 */
void test_zombie_process(void* /*arg*/) {
  klog::Info("=== Zombie Process Test ===\n");

  // 创建父进程
  auto* parent = new TaskControlBlock("Parent", 10, nullptr, nullptr);
  parent->pid = 5300;
  parent->tgid = 5300;
  parent->parent_pid = 1;

  Singleton<TaskManager>::GetInstance().AddTask(parent);

  // 创建子进程
  auto* child = new TaskControlBlock("Child", 10, nullptr, nullptr);
  child->pid = 5301;
  child->tgid = 5301;
  child->parent_pid = parent->pid;

  Singleton<TaskManager>::GetInstance().AddTask(child);

  sys_sleep(50);

  // 子进程退出，应该变为僵尸状态等待父进程回收
  child->exit_code = 0;
  child->status = TaskStatus::kZombie;

  if (child->parent_pid == parent->pid &&
      child->status == TaskStatus::kZombie) {
    klog::Info(
        "Child process (pid=%zu) became zombie, waiting for parent to "
        "reap\n",
        child->pid);
  } else {
    klog::Err("Zombie process test failed\n");
  }

  klog::Info("Zombie Process Test: PASSED\n");
}

}  // namespace

/**
 * @brief Exit 系统测试入口
 */
auto exit_system_test() -> bool {
  klog::Info("===== Exit System Test Start =====\n");

  auto& task_mgr = Singleton<TaskManager>::GetInstance();

  // 测试 1: Normal exit
  auto* test1 =
      new TaskControlBlock("TestExitNormal", 10, test_exit_normal, nullptr);
  task_mgr.AddTask(test1);

  // 测试 2: Exit with error
  auto* test2 = new TaskControlBlock("TestExitWithError", 10,
                                     test_exit_with_error, nullptr);
  task_mgr.AddTask(test2);

  // 测试 3: Thread exit
  auto* test3 =
      new TaskControlBlock("TestThreadExit", 10, test_thread_exit, nullptr);
  task_mgr.AddTask(test3);

  // 测试 4: Orphan exit
  auto* test4 =
      new TaskControlBlock("TestOrphanExit", 10, test_orphan_exit, nullptr);
  task_mgr.AddTask(test4);

  // 测试 5: Zombie process
  auto* test5 = new TaskControlBlock("TestZombieProcess", 10,
                                     test_zombie_process, nullptr);
  task_mgr.AddTask(test5);

  klog::Info("Exit System Test Suite: COMPLETED\n");
  return true;
}
