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

void TaskManager::Block(uint64_t resource_id) {
  auto& cpu_sched = GetCurrentCpuSched();

  auto* current = GetCurrentTask();

  {
    LockGuard<SpinLock> lock_guard(cpu_sched.lock);

    if (!current) {
      klog::Err("Block: No current task to block.\n");
      return;
    }

    // 将任务标记为阻塞状态
    current->status = TaskStatus::kBlocked;

    // 记录阻塞的资源 ID
    current->blocked_on = resource_id;

    // 将任务加入阻塞队列（按资源 ID 分组）
    cpu_sched.blocked_tasks[resource_id].push_back(current);
  }

  // 调度到其他任务
  Schedule();

  // 任务被唤醒后会从这里继续执行
}
