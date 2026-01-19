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
  cpu_sched.lock.lock();

  // 1. 递增本核心的 tick 计数
  cpu_sched.local_tick++;

  auto* current = GetCurrentTask();

  // 2. 唤醒睡眠队列中到时间的任务
  while (!cpu_sched.sleeping_tasks.empty()) {
    auto* task = cpu_sched.sleeping_tasks.top();

    // 检查是否到达唤醒时间
    if (task->sched_info.wake_tick > cpu_sched.local_tick) {
      break;  // 队列按时间排序，后面的任务都还没到时间
    }

    // 唤醒任务
    cpu_sched.sleeping_tasks.pop();
    task->status = TaskStatus::kReady;

    // 将任务重新加入对应调度器的就绪队列
    auto* scheduler = cpu_sched.schedulers[task->policy];
    if (scheduler) {
      scheduler->Enqueue(task);
    }
  }

  // 3. 更新当前任务的统计信息
  if (current && current->status == TaskStatus::kRunning) {
    // 更新总运行时间
    current->sched_info.total_runtime++;

    // 减少剩余时间片
    if (current->sched_info.time_slice_remaining > 0) {
      current->sched_info.time_slice_remaining--;
    }

    // 4. 调用调度器的 OnTick 钩子，检查是否需要抢占
    auto* scheduler = cpu_sched.schedulers[current->policy];
    bool need_preempt = false;

    if (scheduler) {
      // 调度器可能基于自己的策略决定是否抢占
      // 例如 CFS 基于 vruntime，RR 基于时间片
      need_preempt = scheduler->OnTick(current);
    }

    // 5. 检查时间片是否耗尽（对于基于时间片的调度器）
    if (!need_preempt && current->sched_info.time_slice_remaining == 0) {
      need_preempt = true;
    }

    // 6. 如果需要抢占，触发调度
    if (need_preempt) {
      cpu_sched.lock.unlock();
      Schedule();
      return;
    }
  }

  cpu_sched.lock.unlock();
}
