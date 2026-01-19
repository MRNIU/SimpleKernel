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

void TaskManager::Exit(int exit_code) {
  auto& cpu_sched = GetCurrentCpuSched();

  LockGuard<SpinLock> lock_guard(cpu_sched.lock);

  auto* current = GetCurrentTask();
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
    current->status = TaskStatus::kExited;
    /// @todo 释放任务资源
  }

  // 触发调度器
  Schedule();

  // 退出后不应执行到这里
  klog::Err("Exit: Task %zu should not return from Schedule()\n", current->pid);

  __builtin_unreachable();
}
