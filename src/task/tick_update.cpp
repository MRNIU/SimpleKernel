/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <algorithm>
#include <memory>
#include <new>

#include "basic_info.hpp"
#include "fifo_scheduler.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "singleton.hpp"
#include "sk_cstring"
#include "sk_stdlib.h"
#include "sk_vector"
#include "task_manager.hpp"
#include "virtual_memory.hpp"

void TaskManager::TickUpdate() {
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
      // 标记任务需要重新调度，时间片将在 Schedule() 中重置
      current_task->status = TaskStatus::kReady;
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
