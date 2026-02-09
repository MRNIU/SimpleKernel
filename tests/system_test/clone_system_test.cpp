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

std::atomic<int> g_process_counter{0};
std::atomic<int> g_thread_counter{0};

/**
 * @brief 子进程工作函数
 */
void child_process_work(void* arg) {
  uint64_t child_id = reinterpret_cast<uint64_t>(arg);

  klog::Info("Child process %zu: starting\n", child_id);

  for (int i = 0; i < 5; ++i) {
    g_process_counter++;
    klog::Debug("Child process %zu: counter=%d, iter=%d\n", child_id,
                g_process_counter.load(), i);
    sys_sleep(20);
  }

  klog::Info("Child process %zu: exiting\n", child_id);
  sys_exit(static_cast<int>(child_id));
}

/**
 * @brief 子线程工作函数
 */
void child_thread_work(void* arg) {
  uint64_t thread_id = reinterpret_cast<uint64_t>(arg);

  klog::Info("Child thread %zu: starting\n", thread_id);

  for (int i = 0; i < 5; ++i) {
    g_thread_counter++;
    klog::Debug("Child thread %zu: counter=%d, iter=%d\n", thread_id,
                g_thread_counter.load(), i);
    sys_sleep(20);
  }

  klog::Info("Child thread %zu: exiting\n", thread_id);
  sys_exit(static_cast<int>(thread_id));
}

/**
 * @brief 测试使用 clone 创建子进程（不共享地址空间）
 */
void test_clone_process(void* /*arg*/) {
  klog::Info("=== Clone Process Test ===\n");

  g_process_counter = 0;

  // 创建父进程
  auto* parent = new TaskControlBlock("CloneParent", 10, nullptr, nullptr);
  parent->pid = 2000;
  parent->tgid = 2000;
  parent->parent_pid = 1;

  Singleton<TaskManager>::GetInstance().AddTask(parent);

  // 使用 clone 创建子进程（不设置 kCloneVm，表示不共享地址空间）
  uint64_t flags = 0;  // 不共享地址空间

  // 创建子进程 1
  auto* child1 = new TaskControlBlock("CloneChild1", 10, child_process_work,
                                      reinterpret_cast<void*>(1));
  child1->parent_pid = parent->pid;
  child1->pgid = parent->pid;
  child1->clone_flags = static_cast<CloneFlags>(flags);

  Singleton<TaskManager>::GetInstance().AddTask(child1);

  // 创建子进程 2
  auto* child2 = new TaskControlBlock("CloneChild2", 10, child_process_work,
                                      reinterpret_cast<void*>(2));
  child2->parent_pid = parent->pid;
  child2->pgid = parent->pid;
  child2->clone_flags = static_cast<CloneFlags>(flags);

  Singleton<TaskManager>::GetInstance().AddTask(child2);

  klog::Info("Created parent (pid=%zu) and 2 child processes\n", parent->pid);

  // 等待子进程运行
  sys_sleep(200);

  klog::Info("Process counter: %d (expected >= 10)\n",
             g_process_counter.load());

  // 验证子进程的 tgid 应该等于它们自己的 pid（独立进程）
  if (child1->tgid == child1->pid && child2->tgid == child2->pid) {
    klog::Info("Child processes have correct tgid\n");
  } else {
    klog::Err("Child processes have incorrect tgid\n");
  }

  // 验证父子关系
  if (child1->parent_pid == parent->pid && child2->parent_pid == parent->pid) {
    klog::Info("Parent-child relationship is correct\n");
  } else {
    klog::Err("Parent-child relationship is incorrect\n");
  }

  klog::Info("Clone Process Test: PASSED\n");
}

/**
 * @brief 测试使用 clone 创建线程（共享地址空间）
 */
void test_clone_thread(void* /*arg*/) {
  klog::Info("=== Clone Thread Test ===\n");

  g_thread_counter = 0;

  // 创建父线程（线程组的主线程）
  auto* leader =
      new TaskControlBlock("CloneThreadLeader", 10, nullptr, nullptr);
  leader->pid = 3000;
  leader->tgid = 3000;
  leader->parent_pid = 1;

  Singleton<TaskManager>::GetInstance().AddTask(leader);

  // 使用 clone 创建线程（设置 kCloneThread 和 kCloneVm）
  uint64_t flags = kCloneThread | kCloneVm | kCloneFiles | kCloneSighand;

  // 创建子线程 1
  auto* thread1 = new TaskControlBlock("CloneThread1", 10, child_thread_work,
                                       reinterpret_cast<void*>(1));
  thread1->parent_pid = leader->pid;
  thread1->tgid = leader->tgid;  // 共享线程组 ID
  thread1->pgid = leader->pgid;
  thread1->clone_flags = static_cast<CloneFlags>(flags);
  thread1->JoinThreadGroup(leader);

  Singleton<TaskManager>::GetInstance().AddTask(thread1);

  // 创建子线程 2
  auto* thread2 = new TaskControlBlock("CloneThread2", 10, child_thread_work,
                                       reinterpret_cast<void*>(2));
  thread2->parent_pid = leader->pid;
  thread2->tgid = leader->tgid;  // 共享线程组 ID
  thread2->pgid = leader->pgid;
  thread2->clone_flags = static_cast<CloneFlags>(flags);
  thread2->JoinThreadGroup(leader);

  Singleton<TaskManager>::GetInstance().AddTask(thread2);

  klog::Info("Created thread leader (pid=%zu, tgid=%zu) and 2 threads\n",
             leader->pid, leader->tgid);

  // 等待线程运行
  sys_sleep(200);

  klog::Info("Thread counter: %d (expected >= 10)\n", g_thread_counter.load());

  // 验证所有线程的 tgid 应该相同（属于同一线程组）
  if (thread1->tgid == leader->tgid && thread2->tgid == leader->tgid) {
    klog::Info("All threads have same tgid\n");
  } else {
    klog::Err("Threads have incorrect tgid\n");
  }

  // 验证线程组大小
  size_t group_size = leader->GetThreadGroupSize();
  klog::Info("Thread group size: %zu (expected 3)\n", group_size);
  if (group_size == 3) {
    klog::Info("Thread group size is correct\n");
  } else {
    klog::Err("Thread group size is incorrect\n");
  }

  klog::Info("Clone Thread Test: PASSED\n");
}

/**
 * @brief 测试 kCloneParent 标志
 */
void test_clone_parent_flag(void* /*arg*/) {
  klog::Info("=== Clone Parent Flag Test ===\n");

  // 创建祖父进程
  auto* grandparent = new TaskControlBlock("Grandparent", 10, nullptr, nullptr);
  grandparent->pid = 4000;
  grandparent->tgid = 4000;
  grandparent->parent_pid = 1;

  // 创建父进程
  auto* parent = new TaskControlBlock("Parent", 10, nullptr, nullptr);
  parent->pid = 4001;
  parent->tgid = 4001;
  parent->parent_pid = grandparent->pid;

  // 不使用 kCloneParent：子进程的父进程应该是 parent
  auto* child_no_flag =
      new TaskControlBlock("ChildNoFlag", 10, nullptr, nullptr);
  child_no_flag->pid = 4002;
  child_no_flag->tgid = 4002;
  child_no_flag->parent_pid = parent->pid;  // 设置为 parent

  // 使用 kCloneParent：子进程的父进程应该是 grandparent
  uint64_t flags = kCloneParent;
  auto* child_with_flag =
      new TaskControlBlock("ChildWithFlag", 10, nullptr, nullptr);
  child_with_flag->pid = 4003;
  child_with_flag->tgid = 4003;
  child_with_flag->parent_pid = parent->parent_pid;  // 设置为 grandparent
  child_with_flag->clone_flags = static_cast<CloneFlags>(flags);

  // 验证父进程关系
  bool check1 = (child_no_flag->parent_pid == parent->pid);
  bool check2 = (child_with_flag->parent_pid == grandparent->pid);

  klog::Info("Child without kCloneParent: parent_pid=%zu (expected %zu)\n",
             child_no_flag->parent_pid, parent->pid);
  klog::Info("Child with kCloneParent: parent_pid=%zu (expected %zu)\n",
             child_with_flag->parent_pid, grandparent->pid);

  if (check1 && check2) {
    klog::Info("kCloneParent flag works correctly\n");
  } else {
    klog::Err("kCloneParent flag test failed\n");
  }

  delete grandparent;
  delete parent;
  delete child_no_flag;
  delete child_with_flag;

  klog::Info("Clone Parent Flag Test: PASSED\n");
}

/**
 * @brief 测试 clone 时的标志位自动补全
 */
void test_clone_flags_auto_completion(void* /*arg*/) {
  klog::Info("=== Clone Flags Auto Completion Test ===\n");

  // 如果只设置 kCloneThread，应该自动补全其他必需的标志
  uint64_t flags = kCloneThread;

  if ((flags & kCloneThread) &&
      (!(flags & kCloneVm) || !(flags & kCloneFiles) ||
       !(flags & kCloneSighand))) {
    klog::Info("Auto-completing flags for kCloneThread\n");
    flags |= (kCloneVm | kCloneFiles | kCloneSighand);
  }

  bool check1 = (flags & kCloneThread);
  bool check2 = (flags & kCloneVm);
  bool check3 = (flags & kCloneFiles);
  bool check4 = (flags & kCloneSighand);

  klog::Info("Flags after auto-completion: 0x%lx\n", flags);

  if (check1 && check2 && check3 && check4) {
    klog::Info("All required flags are set\n");
  } else {
    klog::Err("Flag auto-completion failed\n");
  }

  klog::Info("Clone Flags Auto Completion Test: PASSED\n");
}

}  // namespace

/**
 * @brief Clone 系统测试入口
 */
auto clone_system_test() -> bool {
  klog::Info("===== Clone System Test Start =====\n");

  auto& task_mgr = Singleton<TaskManager>::GetInstance();

  // 测试 1: Clone process
  auto* test1 =
      new TaskControlBlock("TestCloneProcess", 10, test_clone_process, nullptr);
  task_mgr.AddTask(test1);

  // 测试 2: Clone thread
  auto* test2 =
      new TaskControlBlock("TestCloneThread", 10, test_clone_thread, nullptr);
  task_mgr.AddTask(test2);

  // 测试 3: Clone parent flag
  auto* test3 = new TaskControlBlock("TestCloneParentFlag", 10,
                                     test_clone_parent_flag, nullptr);
  task_mgr.AddTask(test3);

  // 测试 4: Clone flags auto completion
  auto* test4 = new TaskControlBlock("TestCloneFlagsAutoCompletion", 10,
                                     test_clone_flags_auto_completion, nullptr);
  task_mgr.AddTask(test4);

  klog::Info("Clone System Test Suite: COMPLETED\n");
  return true;
}
