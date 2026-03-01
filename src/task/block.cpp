/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cassert>

#include "kernel_log.hpp"
#include "resource_id.hpp"
#include "task_manager.hpp"
#include "task_messages.hpp"

void TaskManager::Block(ResourceId resource_id) {
  auto& cpu_sched = GetCurrentCpuSched();

  auto* current = GetCurrentTask();
  assert(current != nullptr && "Block: No current task to block");
  assert(current->GetStatus() == TaskStatus::kRunning &&
         "Block: current task status must be kRunning");

  {
    LockGuard<SpinLock> lock_guard(cpu_sched.lock);

    // Check capacity before transitioning FSM
    auto& list = cpu_sched.blocked_tasks[resource_id];
    if (list.full()) {
      klog::err
          << "Block: blocked_tasks list full for resource, cannot block task "
          << current->pid << "\\n";
      // Rollback: task stays kRunning, do not transition FSM
      return;
    }
    // Transition: kRunning -> kBlocked
    current->fsm.Receive(MsgBlock{resource_id});
    // Record blocked resource
    current->blocked_on = resource_id;
    list.push_back(current);

    klog::debug() << "Block: pid=" << current->pid
                  << " blocked on resource=" << resource_id.GetTypeName()
                  << ", data=" << klog::hex << resource_id.GetData();
  }

  // 调度到其他任务
  Schedule();

  // 任务被唤醒后会从这里继续执行
}
