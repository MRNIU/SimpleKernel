/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kernel_log.hpp"
#include "task_manager.hpp"

void TaskManager::TickUpdate() {
  auto& cpu_sched = GetCurrentCpuSched();

  bool need_preempt = false;

  {
    LockGuard<SpinLock> lock_guard(cpu_sched.lock);

    // 递增本核心的 tick 计数
    cpu_sched.local_tick++;

    auto* current = GetCurrentTask();

    // 唤醒睡眠队列中到时间的任务
    while (!cpu_sched.sleeping_tasks.empty()) {
      auto* task = cpu_sched.sleeping_tasks.top();

      // 检查是否到达唤醒时间
      if (task->sched_info.wake_tick > cpu_sched.local_tick) {
        // sleeping_tasks 是一个 priority_queue，只需要检查队首元素
        break;
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

    // 更新当前任务的统计信息
    if (current && current->status == TaskStatus::kRunning) {
      // 更新总运行时间
      current->sched_info.total_runtime++;

      // 减少剩余时间片
      if (current->sched_info.time_slice_remaining > 0) {
        current->sched_info.time_slice_remaining--;
      }

      // 调用调度器的 OnTick，检查是否需要抢占
      auto* scheduler = cpu_sched.schedulers[current->policy];

      if (scheduler) {
        // 调度器可能基于自己的策略决定是否抢占
        need_preempt = scheduler->OnTick(current);
      }

      // 检查时间片是否耗尽（对于基于时间片的调度器）
      if (!need_preempt && current->sched_info.time_slice_remaining == 0) {
        need_preempt = true;
      }
    }
  }

  // 如果需要抢占，触发调度
  if (need_preempt) {
    Schedule();
  }
}
