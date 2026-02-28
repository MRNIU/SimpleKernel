/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task_manager.hpp"

#include <cpu_io.h>
#include <etl/vector.h>

#include <algorithm>
#include <cassert>
#include <climits>
#include <memory>
#include <new>

#include "basic_info.hpp"
#include "fifo_scheduler.hpp"
#include "idle_scheduler.hpp"
#include "kernel_config.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "kstd_cstring"
#include "rr_scheduler.hpp"
#include "singleton.hpp"
#include "sk_stdlib.h"
#include "virtual_memory.hpp"

namespace {

/// idle 线程入口函数
void idle_thread(void*) {
  while (1) {
    cpu_io::Pause();
  }
}

}  // namespace

void TaskManager::InitCurrentCore() {
  auto core_id = cpu_io::GetCurrentCoreId();
  auto& cpu_sched = cpu_schedulers_[core_id];

  LockGuard lock_guard{cpu_sched.lock};

  if (!cpu_sched.schedulers[SchedPolicy::kNormal]) {
    cpu_sched.schedulers[SchedPolicy::kRealTime] =
        kstd::make_unique<FifoScheduler>();
    cpu_sched.schedulers[SchedPolicy::kNormal] =
        kstd::make_unique<RoundRobinScheduler>();
    cpu_sched.schedulers[SchedPolicy::kIdle] =
        kstd::make_unique<IdleScheduler>();
  }

  // 关联 PerCpu
  auto& cpu_data = per_cpu::GetCurrentCore();
  cpu_data.sched_data = &cpu_sched;

  // 创建独立的 Idle 线程
  auto* idle_task = new TaskControlBlock("Idle", INT_MAX, idle_thread, nullptr);
  idle_task->status = TaskStatus::kRunning;
  idle_task->policy = SchedPolicy::kIdle;

  // 将 idle 任务加入 Idle 调度器
  if (cpu_sched.schedulers[SchedPolicy::kIdle]) {
    cpu_sched.schedulers[SchedPolicy::kIdle]->Enqueue(idle_task);
  }

  cpu_data.idle_task = idle_task;
  cpu_data.running_task = idle_task;
}

void TaskManager::AddTask(TaskControlBlock* task) {
  assert(task != nullptr && "AddTask: task must not be null");
  assert(task->status == TaskStatus::kUnInit &&
         "AddTask: task status must be kUnInit");
  // 分配 PID
  if (task->pid == 0) {
    task->pid = AllocatePid();
  }

  // 如果 tgid 未设置，则将其设为自己的 pid (单线程进程或线程组的主线程)
  if (task->tgid == 0) {
    task->tgid = task->pid;
  }

  // 加入全局任务表
  {
    LockGuard lock_guard{task_table_lock_};
    if (task_table_.full()) {
      klog::Err("AddTask: task_table_ full, cannot add task (pid=%zu)\n",
                task->pid);
      return;
    }
    task_table_[task->pid] = etl::unique_ptr<TaskControlBlock>(task);
  }

  // 设置任务状态为 kReady
  task->status = TaskStatus::kReady;

  // 简单的负载均衡：如果指定了亲和性，放入对应核心，否则放入当前核心
  // 更复杂的逻辑可以是：寻找最空闲的核心
  size_t target_core = cpu_io::GetCurrentCoreId();

  if (task->cpu_affinity != UINT64_MAX) {
    // 寻找第一个允许的核心
    for (size_t core_id = 0; core_id < SIMPLEKERNEL_MAX_CORE_COUNT; ++core_id) {
      if (task->cpu_affinity & (1UL << core_id)) {
        target_core = core_id;
        break;
      }
    }
  }

  auto& cpu_sched = cpu_schedulers_[target_core];

  {
    LockGuard<SpinLock> lock_guard(cpu_sched.lock);
    if (task->policy < SchedPolicy::kPolicyCount) {
      if (cpu_sched.schedulers[task->policy]) {
        cpu_sched.schedulers[task->policy]->Enqueue(task);
      }
    }
  }

  // 如果是当前核心，且添加了比当前任务优先级更高的任务，触发抢占
  if (target_core == cpu_io::GetCurrentCoreId()) {
    auto& cpu_data = per_cpu::GetCurrentCore();
    TaskControlBlock* current = cpu_data.running_task;
    // 如果当前是 idle 任务，或新任务的策略优先级更高，触发调度
    if (current == cpu_data.idle_task ||
        (current && task->policy < current->policy)) {
      // 注意：这里不能直接调用 Schedule()，因为可能在中断上下文中
      // 实际应该设置一个 need_resched 标志，在中断返回前检查
      // 为简化，这里暂时不做抢占，只在时间片耗尽时调度
    }
  }
}

size_t TaskManager::AllocatePid() { return pid_allocator_.fetch_add(1); }

TaskControlBlock* TaskManager::FindTask(Pid pid) {
  LockGuard lock_guard{task_table_lock_};
  auto it = task_table_.find(pid);
  return (it != task_table_.end()) ? it->second.get() : nullptr;
}

void TaskManager::Balance() {
  // 算法留空
  // TODO: 检查其他核心的运行队列长度，如果比当前核心长，则窃取任务
}

void TaskManager::ReapTask(TaskControlBlock* task) {
  if (!task) {
    return;
  }

  // 确保任务处于僵尸或退出状态
  if (task->status != TaskStatus::kZombie &&
      task->status != TaskStatus::kExited) {
    klog::Warn("ReapTask: Task %zu is not in zombie/exited state\n", task->pid);
    return;
  }

  // Capture pid before erase (unique_ptr deletes on erase)
  Pid pid = task->pid;

  // 从全局任务表中移除 (unique_ptr auto-deletes TCB)
  {
    LockGuard lock_guard{task_table_lock_};
    task_table_.erase(pid);
  }

  klog::Debug("ReapTask: Task %zu resources freed\n", pid);
}

void TaskManager::ReparentChildren(TaskControlBlock* parent) {
  if (!parent) {
    return;
  }

  // init 进程的 PID 通常是 1
  /// @todo 当前的 pid 是自增的，需要考虑多核情况
  constexpr Pid kInitPid = 1;

  LockGuard lock_guard{task_table_lock_};

  // 遍历所有任务，找到父进程是当前任务的子进程
  for (auto& [pid, task] : task_table_) {
    if (task && task->parent_pid == parent->pid) {
      // 将子进程过继给 init 进程
      task->parent_pid = kInitPid;
      klog::Debug("ReparentChildren: Task %zu reparented to init (PID %zu)\n",
                  task->pid, kInitPid);

      // 如果子进程已经是僵尸状态，通知 init 进程回收
      /// @todo 实现向 init 进程发送 SIGCHLD 信号
    }
  }
}

etl::vector<TaskControlBlock*, kernel::config::kMaxReadyTasks>
TaskManager::GetThreadGroup(Pid tgid) {
  etl::vector<TaskControlBlock*, kernel::config::kMaxReadyTasks> result;

  LockGuard lock_guard(task_table_lock_);

  // 遍历任务表，找到所有 tgid 匹配的线程
  for (auto& [pid, task] : task_table_) {
    if (task && task->tgid == tgid) {
      result.push_back(task.get());
    }
  }

  return result;
}

void TaskManager::SignalThreadGroup(Pid tgid, int signal) {
  /// @todo 实现信号机制后，向线程组中的所有线程发送信号
  klog::Debug("SignalThreadGroup: tgid=%zu, signal=%d (not implemented)\n",
              tgid, signal);

  // 预期实现：
  // auto threads = GetThreadGroup(tgid);
  // for (auto* thread : threads) {
  //   SendSignal(thread, signal);
  // }
}

TaskManager::~TaskManager() {
  // unique_ptr in cpu_schedulers_.schedulers[] auto-deletes on destruction
}
