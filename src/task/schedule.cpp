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

void TaskManager::Schedule() {
  auto& cpu_sched = GetCurrentCpuSched();
  auto& cpu_data = per_cpu::GetCurrentCore();

  // 必须关中断并持有锁才能操作运行队列
  cpu_sched.lock.lock();

  TaskControlBlock* current_task = cpu_data.running_task;
  TaskControlBlock* next_task = nullptr;

  // 处理当前任务的状态
  if (current_task) {
    switch (current_task->status) {
      case TaskStatus::kExited: {
        // 标记为僵尸状态（等待父进程回收）
        current_task->status = TaskStatus::kZombie;
        // TODO: 通知父进程回收资源，或在没有父进程时直接释放
        // 已退出的任务不会再被放回就绪队列，所以不会再被调度
        break;
      }
      case TaskStatus::kRunning: {
        // Yield的情况，将其放回就绪队列
        current_task->status = TaskStatus::kReady;
        // 不要在此时将 idle task 放回队列，因为它特殊处理
        if (current_task != cpu_data.idle_task) {
          if (current_task->policy < SchedPolicy::kPolicyCount &&
              cpu_sched.schedulers[current_task->policy]) {
            cpu_sched.schedulers[current_task->policy]->Enqueue(current_task);
          }
        }
        break;
      }
      case TaskStatus::kReady: {
        // 时间片耗尽的情况，放回就绪队列
        // 不要在此时将 idle task 放回队列，因为它特殊处理
        if (current_task != cpu_data.idle_task) {
          if (current_task->policy < SchedPolicy::kPolicyCount &&
              cpu_sched.schedulers[current_task->policy]) {
            cpu_sched.schedulers[current_task->policy]->Enqueue(current_task);
          }
        }
        break;
      }

      default: {
        // 其他状态不需要处理
        break;
      }
    }
  }

  // 按优先级遍历调度器，寻找下一个任务
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

  // 如果不是 idle 任务，重置时间片
  if (next_task != cpu_data.idle_task) {
    next_task->time_slice_remaining = next_task->time_slice_default;
  }

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
