/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kernel_log.hpp"
#include "task_manager.hpp"

void TaskManager::Exit(int exit_code) {
  auto& cpu_sched = GetCurrentCpuSched();
  auto* current = GetCurrentTask();

  {
    LockGuard<SpinLock> lock_guard(cpu_sched.lock);

    if (!current) {
      klog::Err("Exit: No current task to exit.\n");
      while (1) {
        ;
      }
    }

    // 设置退出码
    current->exit_code = exit_code;

    if (current->parent_pid != 0) {
      // 有父进程，进入僵尸状态等待回收
      current->status = TaskStatus::kZombie;
      /// @todo 通知父进程
    } else {
      // 没有父进程，直接退出并释放资源
      ReapTask(current);
      current->status = TaskStatus::kExited;
    }
  }

  Schedule();

  // 退出后不应执行到这里
  klog::Err("Exit: Task %zu should not return from Schedule()\n", current->pid);

  __builtin_unreachable();
}
