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
  auto& cpu_data = per_cpu::GetCurrentCore();
  TaskControlBlock* current = cpu_data.running_task;

  if (!current || current == cpu_data.idle_task) {
    return;
  }

  // 加锁修改状态
  cpu_sched.lock.lock();
  current->status = TaskStatus::kExited;
  current->exit_code = exit_code;
  cpu_sched.lock.unlock();

  // 调度到下一个任务，当前任务不会再被调度
  Schedule();
}
