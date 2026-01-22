/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kernel_log.hpp"
#include "resource_id.hpp"
#include "task_manager.hpp"

void TaskManager::Block(ResourceId resource_id) {
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

    klog::Debug("Block: pid=%zu blocked on resource=%s, data=0x%lx\n",
                current->pid, resource_id.GetTypeName(), resource_id.GetData());
  }

  // 调度到其他任务
  Schedule();

  // 任务被唤醒后会从这里继续执行
}
