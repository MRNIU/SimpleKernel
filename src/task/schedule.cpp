/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <algorithm>
#include <memory>
#include <new>

#include "basic_info.hpp"
#include "fifo_scheduler.hpp"
#include "interrupt_base.h"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
#include "singleton.hpp"
#include "sk_cstring"
#include "sk_stdlib.h"
#include "sk_vector"
#include "spinlock.hpp"
#include "task_manager.hpp"
#include "virtual_memory.hpp"

void TaskManager::Schedule() {
  auto& cpu_sched = GetCurrentCpuSched();
  cpu_sched.lock.lock();

  auto* current = GetCurrentTask();

  // 1. 处理当前任务状态
  // 注意：Schedule() 被调用意味着需要调度决策，可能是：
  // - 时间片耗尽（TickUpdate 检测到需要抢占）
  // - 主动让出 CPU (yield)
  // - 任务阻塞、睡眠或退出
  if (current && current->status == TaskStatus::kRunning) {
    // 将当前任务标记为就绪并重新入队（如果它还能运行）
    current->status = TaskStatus::kReady;
    auto* scheduler = cpu_sched.schedulers[current->policy];

    if (scheduler) {
      scheduler->OnPreempted(current);

      // 调度器决定如何处理被抢占的任务
      // 大多数情况下需要重新入队，除非是特殊策略
      if (scheduler->OnTimeSliceExpired(current)) {
        scheduler->Enqueue(current);
      }
    }
  }

  // 2. 选择下一个任务 (按策略优先级: RealTime > Normal > Idle)
  TaskControlBlock* next = nullptr;
  for (size_t i = 0; i < SchedPolicy::kPolicyCount; ++i) {
    auto* scheduler = cpu_sched.schedulers[i];
    if (scheduler && !scheduler->IsEmpty()) {
      next = scheduler->PickNext();
      if (next) {
        break;
      }
    }
  }

  // 3. 如果没有任务可运行
  if (!next) {
    // 如果当前任务仍然可以运行，继续运行它
    if (current && current->status == TaskStatus::kReady) {
      next = current;
    } else {
      // 否则统计空闲时间并返回
      cpu_sched.idle_time++;
      cpu_sched.lock.unlock();
      return;
    }
  }

  // 4. 切换到下一个任务
  next->status = TaskStatus::kRunning;
  // 重置时间片（对于 RR 和 FIFO 有效，CFS 使用 vruntime 不依赖此字段）
  next->sched_info.time_slice_remaining = next->sched_info.time_slice_default;
  next->sched_info.context_switches++;
  cpu_sched.total_schedules++;

  // 调用调度器钩子
  auto* scheduler = cpu_sched.schedulers[next->policy];
  if (scheduler) {
    scheduler->OnScheduled(next);
  }

  // 更新 per-CPU running_task
  per_cpu::GetCurrentCore().running_task = next;

  cpu_sched.lock.unlock();

  // 5. 上下文切换（仅在任务真正改变时）
  if (current && current != next) {
    switch_to(&current->task_context, &next->task_context);
  }
}
