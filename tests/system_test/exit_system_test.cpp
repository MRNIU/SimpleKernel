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
#include "task_messages.hpp"

namespace {

std::atomic<int> g_exit_test_counter{0};
std::atomic<int> g_tests_completed{0};
std::atomic<int> g_tests_failed{0};

// ---------------------------------------------------------------------------
// test_exit_normal
// 测试: 创建带工作函数的任务，让其运行完毕后，验证 TCB 状态字段的语义正确性。
// 具体检查: 初始状态不是 kExited/kZombie，退出码字段可正确写入并读出。
// ---------------------------------------------------------------------------

void normal_work(void* arg) {
  auto* flag = reinterpret_cast<std::atomic<int>*>(arg);
  klog::Debug("normal_work: running\n");
  sys_sleep(30);
  *flag = 1;
  klog::Debug("normal_work: done, calling sys_exit(0)\n");
  sys_exit(0);
}

void test_exit_normal(void* /*arg*/) {
  klog::Info("=== Exit Normal Test ===\n");

  bool passed = true;

  // 1. 创建 TCB 并检查初始状态不是终止态
  auto* task = new TaskControlBlock("ExitNormal", 10, nullptr, nullptr);
  task->pid = 5000;
  task->tgid = 5000;
  task->parent_pid = 1;

  if (task->GetStatus() == TaskStatus::kExited ||
      task->GetStatus() == TaskStatus::kZombie) {
    klog::Err("test_exit_normal: FAIL — fresh TCB already in terminal state\n");
    passed = false;
  }

  // 2. exit_code 默认应为 0
  if (task->exit_code != 0) {
    klog::Err("test_exit_normal: FAIL — default exit_code != 0 (got %d)\n",
              task->exit_code);
    passed = false;
  }

  // 3. 创建有实际工作函数的任务，等待其完成后通过 flag 验证执行路径
  std::atomic<int> work_flag{0};
  auto* worker = new TaskControlBlock("ExitNormalWorker", 10, normal_work,
                                      reinterpret_cast<void*>(&work_flag));
  TaskManagerSingleton::instance().AddTask(worker);

  // 等待 worker 运行完毕（最多 500ms）
  int timeout = 10;
  while (timeout > 0 && work_flag.load() == 0) {
    sys_sleep(50);
    timeout--;
  }

  if (work_flag.load() != 1) {
    klog::Err("test_exit_normal: FAIL — worker did not complete\n");
    passed = false;
  } else {
    klog::Info("test_exit_normal: worker completed successfully\n");
  }

  // 4. 向 task 写入退出信息并验证读回一致
  task->exit_code = 0;
  task->fsm.Receive(MsgSchedule{});     // kUnInit -> kReady
  task->fsm.Receive(MsgSchedule{});     // kReady -> kRunning
  task->fsm.Receive(MsgExit{0, true});  // kRunning -> kZombie
  if (task->exit_code != 0 || task->GetStatus() != TaskStatus::kZombie) {
    klog::Err("test_exit_normal: FAIL — TCB field write-back mismatch\n");
    passed = false;
  }

  if (passed) {
    klog::Info("Exit Normal Test: PASSED\n");
  } else {
    klog::Err("Exit Normal Test: FAILED\n");
    g_tests_failed++;
  }

  g_tests_completed++;
  sys_exit(passed ? 0 : 1);
}

// ---------------------------------------------------------------------------
// test_exit_with_error
// 测试: 验证非零退出码可被正确存储和读回；字段语义与正常退出对称。
// ---------------------------------------------------------------------------

void error_work(void* arg) {
  auto* flag = reinterpret_cast<std::atomic<int>*>(arg);
  klog::Debug("error_work: running\n");
  sys_sleep(30);
  *flag = 42;
  klog::Debug("error_work: done, calling sys_exit(42)\n");
  sys_exit(42);
}

void test_exit_with_error(void* /*arg*/) {
  klog::Info("=== Exit With Error Test ===\n");

  bool passed = true;

  // 1. 创建 TCB，确认 exit_code 默认为 0
  auto* task = new TaskControlBlock("ExitError", 10, nullptr, nullptr);
  task->pid = 5001;
  task->tgid = 5001;
  task->parent_pid = 1;

  if (task->exit_code != 0) {
    klog::Err("test_exit_with_error: FAIL — default exit_code != 0 (got %d)\n",
              task->exit_code);
    passed = false;
  }

  // 2. 创建带实际工作的任务，以错误码退出
  std::atomic<int> work_flag{0};
  auto* worker = new TaskControlBlock("ExitErrorWorker", 10, error_work,
                                      reinterpret_cast<void*>(&work_flag));
  TaskManagerSingleton::instance().AddTask(worker);

  int timeout = 10;
  while (timeout > 0 && work_flag.load() == 0) {
    sys_sleep(50);
    timeout--;
  }

  if (work_flag.load() != 42) {
    klog::Err("test_exit_with_error: FAIL — worker did not set error flag\n");
    passed = false;
  } else {
    klog::Info("test_exit_with_error: worker set flag to %d\n",
               work_flag.load());
  }

  // 3. 验证 TCB 中的退出码字段可以正确存储非零值
  task->exit_code = 42;
  task->fsm.Receive(MsgSchedule{});      // kUnInit -> kReady
  task->fsm.Receive(MsgSchedule{});      // kReady -> kRunning
  task->fsm.Receive(MsgExit{42, true});  // kRunning -> kZombie
  if (task->exit_code != 42) {
    klog::Err(
        "test_exit_with_error: FAIL — exit_code write-back mismatch "
        "(expected 42, got %d)\n",
        task->exit_code);
    passed = false;
  }
  if (task->GetStatus() != TaskStatus::kZombie) {
    klog::Err("test_exit_with_error: FAIL — status write-back mismatch\n");
    passed = false;
  }

  if (passed) {
    klog::Info("Exit With Error Test: PASSED\n");
  } else {
    klog::Err("Exit With Error Test: FAILED\n");
    g_tests_failed++;
  }

  g_tests_completed++;
  sys_exit(passed ? 0 : 1);
}

// ---------------------------------------------------------------------------
// test_thread_exit (ALREADY GOOD — kept with counter increment added)
// ---------------------------------------------------------------------------

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

  TaskManagerSingleton::instance().AddTask(leader);

  // 创建子线程
  auto* thread1 = new TaskControlBlock("Thread1", 10, child_thread_exit_work,
                                       reinterpret_cast<void*>(1));
  thread1->pid = 5101;
  thread1->tgid = 5100;
  thread1->JoinThreadGroup(leader);

  TaskManagerSingleton::instance().AddTask(thread1);

  auto* thread2 = new TaskControlBlock("Thread2", 10, child_thread_exit_work,
                                       reinterpret_cast<void*>(2));
  thread2->pid = 5102;
  thread2->tgid = 5100;
  thread2->JoinThreadGroup(leader);

  TaskManagerSingleton::instance().AddTask(thread2);

  klog::Info("Created thread group with leader (pid=%zu) and 2 threads\n",
             leader->pid);

  // 等待线程运行并退出
  sys_sleep(200);

  klog::Info("Exit test counter: %d (expected >= 6)\n",
             g_exit_test_counter.load());

  bool passed = (g_exit_test_counter.load() >= 6);

  if (passed) {
    klog::Info("Thread Exit Test: PASSED\n");
  } else {
    klog::Err("Thread Exit Test: FAILED (counter=%d, expected >= 6)\n",
              g_exit_test_counter.load());
    g_tests_failed++;
  }

  g_tests_completed++;
  sys_exit(passed ? 0 : 1);
}

// ---------------------------------------------------------------------------
// test_orphan_exit
// 测试: 孤儿进程 (parent_pid == 0) 的 TCB 字段语义：
//   - parent_pid 正确存储 0
//   - 无父进程时退出应进入 kExited 而非 kZombie
// ---------------------------------------------------------------------------

void orphan_work(void* arg) {
  auto* flag = reinterpret_cast<std::atomic<int>*>(arg);
  klog::Debug("orphan_work: running\n");
  sys_sleep(30);
  *flag = 1;
  klog::Debug("orphan_work: done\n");
  sys_exit(0);
}

void test_orphan_exit(void* /*arg*/) {
  klog::Info("=== Orphan Exit Test ===\n");

  bool passed = true;

  // 1. 创建孤儿 TCB，验证 parent_pid == 0 被正确存储
  auto* orphan = new TaskControlBlock("Orphan", 10, nullptr, nullptr);
  orphan->pid = 5200;
  orphan->tgid = 5200;
  orphan->parent_pid = 0;  // 孤儿进程

  if (orphan->parent_pid != 0) {
    klog::Err("test_orphan_exit: FAIL — parent_pid not stored as 0 (got %zu)\n",
              orphan->parent_pid);
    passed = false;
  }

  // 2. 孤儿进程退出时预期进入 kExited 而非 kZombie（无父进程等待回收）
  orphan->exit_code = 0;
  orphan->fsm.Receive(MsgSchedule{});      // kUnInit -> kReady
  orphan->fsm.Receive(MsgSchedule{});      // kReady -> kRunning
  orphan->fsm.Receive(MsgExit{0, false});  // kRunning -> kExited (no parent)
  if (orphan->GetStatus() != TaskStatus::kExited) {
    klog::Err(
        "test_orphan_exit: FAIL — orphan status should be kExited "
        "(got %d)\n",
        static_cast<int>(orphan->GetStatus()));
    passed = false;
  }
  if (orphan->parent_pid != 0) {
    klog::Err("test_orphan_exit: FAIL — parent_pid changed unexpectedly\n");
    passed = false;
  }

  // 3. 用实际工作函数验证孤儿任务能正常执行和退出
  std::atomic<int> work_flag{0};
  auto* orphan_worker = new TaskControlBlock(
      "OrphanWorker", 10, orphan_work, reinterpret_cast<void*>(&work_flag));
  orphan_worker->parent_pid = 0;  // 无父进程
  TaskManagerSingleton::instance().AddTask(orphan_worker);

  int timeout = 10;
  while (timeout > 0 && work_flag.load() == 0) {
    sys_sleep(50);
    timeout--;
  }

  if (work_flag.load() != 1) {
    klog::Err("test_orphan_exit: FAIL — orphan worker did not complete\n");
    passed = false;
  } else {
    klog::Info("test_orphan_exit: orphan worker completed\n");
  }

  if (passed) {
    klog::Info("Orphan Exit Test: PASSED\n");
  } else {
    klog::Err("Orphan Exit Test: FAILED\n");
    g_tests_failed++;
  }

  g_tests_completed++;
  sys_exit(passed ? 0 : 1);
}

// ---------------------------------------------------------------------------
// test_zombie_process
// 测试: 子进程退出后变为僵尸状态——TCB 语义契约：
//   - parent_pid 指向父进程
//   - 退出后应是 kZombie（等父进程回收）而非 kExited
// ---------------------------------------------------------------------------

void child_work(void* arg) {
  auto* flag = reinterpret_cast<std::atomic<int>*>(arg);
  klog::Debug("child_work: running\n");
  sys_sleep(30);
  *flag = 1;
  klog::Debug("child_work: done\n");
  sys_exit(0);
}

void test_zombie_process(void* /*arg*/) {
  klog::Info("=== Zombie Process Test ===\n");

  bool passed = true;

  // 1. 创建父子 TCB，验证 parent_pid 字段正确关联
  auto* parent = new TaskControlBlock("Parent", 10, nullptr, nullptr);
  parent->pid = 5300;
  parent->tgid = 5300;
  parent->parent_pid = 1;

  TaskManagerSingleton::instance().AddTask(parent);

  auto* child = new TaskControlBlock("Child", 10, nullptr, nullptr);
  child->pid = 5301;
  child->tgid = 5301;
  child->parent_pid = parent->pid;

  if (child->parent_pid != parent->pid) {
    klog::Err(
        "test_zombie_process: FAIL — child parent_pid mismatch "
        "(expected %zu, got %zu)\n",
        parent->pid, child->parent_pid);
    passed = false;
  }

  TaskManagerSingleton::instance().AddTask(child);

  // 2. 子进程退出时，因父进程仍存在，应变为 kZombie（等待父进程 wait）
  child->exit_code = 0;
  child->fsm.Receive(MsgSchedule{});     // kUnInit -> kReady
  child->fsm.Receive(MsgSchedule{});     // kReady -> kRunning
  child->fsm.Receive(MsgExit{0, true});  // kRunning -> kZombie (has parent)
  if (child->GetStatus() != TaskStatus::kZombie) {
    klog::Err(
        "test_zombie_process: FAIL — child with living parent should be "
        "kZombie (got %d)\n",
        static_cast<int>(child->GetStatus()));
    passed = false;
  }
  if (child->parent_pid != parent->pid) {
    klog::Err(
        "test_zombie_process: FAIL — child parent_pid changed after "
        "status update\n");
    passed = false;
  }

  klog::Info(
      "Child process (pid=%zu) became zombie, waiting for parent to "
      "reap\n",
      child->pid);

  // 3. 用真实工作函数验证有父进程的子任务能正常执行
  std::atomic<int> work_flag{0};
  auto* real_child = new TaskControlBlock("RealChild", 10, child_work,
                                          reinterpret_cast<void*>(&work_flag));
  real_child->parent_pid = 5300;  // 指向 parent
  TaskManagerSingleton::instance().AddTask(real_child);

  int timeout = 10;
  while (timeout > 0 && work_flag.load() == 0) {
    sys_sleep(50);
    timeout--;
  }

  if (work_flag.load() != 1) {
    klog::Err("test_zombie_process: FAIL — child worker did not complete\n");
    passed = false;
  } else {
    klog::Info("test_zombie_process: child worker completed\n");
  }

  if (passed) {
    klog::Info("Zombie Process Test: PASSED\n");
  } else {
    klog::Err("Zombie Process Test: FAILED\n");
    g_tests_failed++;
  }

  g_tests_completed++;
  sys_exit(passed ? 0 : 1);
}

}  // namespace

/**
 * @brief Exit 系统测试入口
 */
auto exit_system_test() -> bool {
  klog::Info("===== Exit System Test Start =====\n");

  // 重置全局计数器
  g_tests_completed = 0;
  g_tests_failed = 0;
  g_exit_test_counter = 0;

  auto& task_mgr = TaskManagerSingleton::instance();

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

  klog::Info("Waiting for all 5 sub-tests to complete...\n");

  // 等待所有子测试完成（每个子测试在退出前会增加 g_tests_completed）
  // 超时: 200 * 50ms = 10s
  int timeout = 200;
  while (timeout > 0) {
    sys_sleep(50);
    if (g_tests_completed.load() >= 5) {
      break;
    }
    timeout--;
  }

  klog::Info("Exit System Test: completed=%d, failed=%d\n",
             g_tests_completed.load(), g_tests_failed.load());

  EXPECT_EQ(g_tests_completed, 5, "All 5 sub-tests completed");
  EXPECT_EQ(g_tests_failed, 0, "No sub-tests failed");

  klog::Info("===== Exit System Test End =====\n");
  return true;
}
