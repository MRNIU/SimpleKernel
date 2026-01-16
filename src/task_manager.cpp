/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task_manager.hpp"

#include <cpu_io.h>

#include <algorithm>
#include <memory>
#include <new>

#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "scheduler/fifo_scheduler.hpp"
#include "singleton.hpp"
#include "sk_cstring"
#include "sk_stdlib.h"
#include "sk_vector"
#include "virtual_memory.hpp"

void TaskManager::InitCurrentCore() {
  // 初始化每个核心的调度器
  size_t core_id = cpu_io::GetCurrentCoreId();
  auto& cpu_sched = cpu_schedulers_[core_id];

  if (!cpu_sched.schedulers[SchedPolicy::kNormal]) {
    cpu_sched.schedulers[SchedPolicy::kRealTime] = new RtScheduler();
    cpu_sched.schedulers[SchedPolicy::kNormal] = new FifoScheduler();
    // Idle 策略可以有一个专门的调度器，或者直接使用 idle_task
    cpu_sched.schedulers[SchedPolicy::kIdle] = nullptr;
  }

  // 关联 PerCpu
  auto& cpu_data = per_cpu::GetCurrentCore();
  cpu_data.sched_data = &cpu_sched;

  auto* main_task = new TaskControlBlock();
  main_task->name = "Idle/Main";

  // 所有核心的 Idle 任务都使用 PID 0
  main_task->pid = 0;

  main_task->status = TaskStatus::kRunning;
  main_task->policy = SchedPolicy::kIdle;
  main_task->cpu_affinity = (1UL << core_id);

  cpu_data.running_task = main_task;
  cpu_data.idle_task = main_task;
}

void TaskManager::AddTask(TaskControlBlock* task) {
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
}

void TaskManager::Schedule() {
  auto& cpu_sched = GetCurrentCpuSched();
  auto& cpu_data = per_cpu::GetCurrentCore();

  // 必须关中断并持有锁才能操作运行队列
  cpu_sched.lock.lock();

  TaskControlBlock* current_task = cpu_data.running_task;
  TaskControlBlock* next_task = nullptr;

  // 1. 如果当前任务还在运行 (Yield的情况)，先将其放回就绪队列
  if (current_task && current_task->status == TaskStatus::kRunning) {
    current_task->status = TaskStatus::kReady;
    // 不要在此时将 idle task 放回队列，因为它特殊处理
    if (current_task != cpu_data.idle_task) {
      if (current_task->policy < SchedPolicy::kPolicyCount &&
          cpu_sched.schedulers[current_task->policy]) {
        cpu_sched.schedulers[current_task->policy]->Enqueue(current_task);
      }
    }
  }

  // 2. 按优先级遍历调度器，寻找下一个任务
  for (auto* sched : cpu_sched.schedulers) {
    if (sched) {
      next_task = sched->PickNext();
      if (next_task) {
        break;
      }
    }
  }

  if (!next_task) {
    // 如果没有普通任务，尝试从其他核心窃取 (Work Stealing)
    // 暂时需解锁以避免死锁 (如果 Balance 内部需要获取其他锁)
    cpu_sched.lock.unlock();
    Balance();
    cpu_sched.lock.lock();

    // 再次尝试获取
    for (auto* sched : cpu_sched.schedulers) {
      if (sched) {
        next_task = sched->PickNext();
        if (next_task) {
          break;
        }
      }
    }
  }

  if (!next_task) {
    // 仍然没有任务，运行 Idle 任务
    // 如果当前就是 Idle 且 Ready/Running，继续运行
    next_task = cpu_data.idle_task;

    // 如果连 idle task 都没有 (启动阶段?)，则需要 panic 或者等待
    if (!next_task) {
      // Should not happen after InitMainThread
      cpu_sched.lock.unlock();
      return;
    }
  }

  TaskControlBlock* prev_task = current_task;
  cpu_data.running_task = next_task;
  next_task->status = TaskStatus::kRunning;

  // 更新调度统计
  cpu_sched.total_schedules++;

  // 如果任务变了，记录上下文切换
  if (prev_task != next_task) {
    if (prev_task) {
      prev_task->context_switches++;
    }
    next_task->context_switches++;
  }

  cpu_sched.lock.unlock();

  // 如果任务变了，切换
  if (prev_task != next_task) {
    switch_to(&prev_task->task_context, &next_task->task_context);
  }
}

size_t TaskManager::AllocatePid() { return pid_allocator.fetch_add(1); }

void TaskManager::UpdateTick() {
  auto& cpu_sched = GetCurrentCpuSched();
  auto& cpu_data = per_cpu::GetCurrentCore();

  cpu_sched.lock.lock();

  // 更新本核心的 tick 计数
  cpu_sched.local_tick++;

  // 更新当前任务的运行时间和时间片
  TaskControlBlock* current_task = cpu_data.running_task;
  if (current_task && current_task != cpu_data.idle_task) {
    current_task->total_runtime++;

    // 时间片递减
    if (current_task->time_slice_remaining > 0) {
      current_task->time_slice_remaining--;
    }

    // 时间片耗尽，标记需要调度
    if (current_task->time_slice_remaining == 0) {
      // 重置时间片，稍后在 Schedule() 中会将任务放回就绪队列
      current_task->time_slice_remaining = current_task->time_slice_default;
      cpu_sched.lock.unlock();
      Schedule();
      return;
    }
  } else if (current_task == cpu_data.idle_task) {
    // 统计空闲时间
    cpu_sched.idle_time++;
  }

  // 检查睡眠队列，唤醒到期的任务
  while (!cpu_sched.sleeping_tasks.empty()) {
    TaskControlBlock* task = cpu_sched.sleeping_tasks.top();
    if (cpu_sched.local_tick >= task->wake_tick) {
      task->status = TaskStatus::kReady;
      // 唤醒到本核心队列 (或者根据 Affinity)
      // 简单起见，因为是在本核心 sleep 的，wake 到本核心
      if (task->policy < SchedPolicy::kPolicyCount &&
          cpu_sched.schedulers[task->policy]) {
        cpu_sched.schedulers[task->policy]->Enqueue(task);
      }

      cpu_sched.sleeping_tasks.pop();
    } else {
      break;
    }
  }

  cpu_sched.lock.unlock();

  // 更新全局系统时间
  system_boot_tick.fetch_add(1, std::memory_order_relaxed);
}

void TaskManager::Sleep(uint64_t ms) {
  auto& cpu_sched = GetCurrentCpuSched();
  auto& cpu_data = per_cpu::GetCurrentCore();
  TaskControlBlock* current = cpu_data.running_task;

  if (!current || current == cpu_data.idle_task) {
    return;
  }

  // 至少睡眠 1 tick
  uint64_t ticks = ms * tick_frequency / 1000;
  if (ticks == 0) {
    ticks = 1;
  }

  // 加锁修改状态
  cpu_sched.lock.lock();
  // 使用本核心的 local_tick
  current->wake_tick = cpu_sched.local_tick + ticks;
  current->status = TaskStatus::kSleeping;
  cpu_sched.sleeping_tasks.push(current);
  cpu_sched.lock.unlock();

  Schedule();
}

void TaskManager::Balance() {
  // 算法留空
  // TODO: 检查其他核心的运行队列长度，如果比当前核心长，则窃取任务
}
