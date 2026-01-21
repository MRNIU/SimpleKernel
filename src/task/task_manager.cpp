/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task_manager.hpp"

#include <cpu_io.h>

#include <algorithm>
#include <memory>
#include <new>

#include "basic_info.hpp"
#include "fifo_scheduler.hpp"
#include "idle_scheduler.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "rr_scheduler.hpp"
#include "singleton.hpp"
#include "sk_cstring"
#include "sk_stdlib.h"
#include "sk_vector"
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
    cpu_sched.schedulers[SchedPolicy::kRealTime] = new FifoScheduler();
    cpu_sched.schedulers[SchedPolicy::kNormal] = new RoundRobinScheduler();
    cpu_sched.schedulers[SchedPolicy::kIdle] = new IdleScheduler();
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
  // 确保任务状态为 kReady
  if (task->status == TaskStatus::kUnInit) {
    task->status = TaskStatus::kReady;
  }

  // 简单的负载均衡：如果指定了亲和性，放入对应核心，否则放入当前核心
  // 更复杂的逻辑可以是：寻找最空闲的核心
  size_t target_core = cpu_io::GetCurrentCoreId();

  if (task->cpu_affinity != UINT64_MAX) {
    // 寻找第一个允许的核心
    for (size_t i = 0; i < SIMPLEKERNEL_MAX_CORE_COUNT; ++i) {
      if (task->cpu_affinity & (1UL << i)) {
        target_core = i;
        break;
      }
    }
  }

  auto& cpu_sched = cpu_schedulers_[target_core];

  // 加锁保护运行队列
  cpu_sched.lock.lock();

  if (task->policy < SchedPolicy::kPolicyCount) {
    if (cpu_sched.schedulers[task->policy]) {
      cpu_sched.schedulers[task->policy]->Enqueue(task);
    }
  }

  cpu_sched.lock.unlock();

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

size_t TaskManager::AllocatePid() { return pid_allocator.fetch_add(1); }

void TaskManager::Balance() {
  // 算法留空
  // TODO: 检查其他核心的运行队列长度，如果比当前核心长，则窃取任务
}

TaskManager::~TaskManager() {
  // 清理每个核心的调度器
  for (auto& cpu_sched : cpu_schedulers_) {
    for (auto& scheduler : cpu_sched.schedulers) {
      delete scheduler;
      scheduler = nullptr;
    }
  }
}
