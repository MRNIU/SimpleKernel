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

void TaskManager::Wakeup(uint64_t resource_id) {
  auto& cpu_sched = GetCurrentCpuSched();

  LockGuard<SpinLock> lock_guard(cpu_sched.lock);

  // 查找等待该资源的任务列表
  auto it = cpu_sched.blocked_tasks.find(resource_id);

  if (it == cpu_sched.blocked_tasks.end()) {
    // 没有任务等待该资源
    return;
  }

  // 唤醒所有等待该资源的任务
  auto& waiting_tasks = it->second;

  while (!waiting_tasks.empty()) {
    auto* task = waiting_tasks.front();
    waiting_tasks.pop_front();

    // 将任务标记为就绪
    task->status = TaskStatus::kReady;
    task->blocked_on = 0;

    // 将任务重新加入对应调度器的就绪队列
    auto* scheduler = cpu_sched.schedulers[task->policy];
    if (scheduler) {
      scheduler->Enqueue(task);
    }
  }

  // 移除空的资源队列
  cpu_sched.blocked_tasks.erase(resource_id);
}
