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

void TaskManager::Sleep(uint64_t ms) {
  auto& cpu_sched = GetCurrentCpuSched();
  auto& cpu_data = per_cpu::GetCurrentCore();
  TaskControlBlock* current = cpu_data.running_task;

  if (!current || current == cpu_data.idle_task) {
    return;
  }

  // 至少睡眠 1 tick
  uint64_t ticks = ms * SIMPLEKERNEL_TICK / 1000;
  if (ticks == 0) {
    ticks = 1;
  }

  // 加锁修改状态
  cpu_sched.lock.lock();
  // 使用本核心的 local_tick
  current->wake_tick = cpu_sched.local_tick + ticks;
  current->status = TaskStatus::kSleeping;
  cpu_sched.sleeping_tasks.push(current);
  cpu_sched.lock.unlock();

  Schedule();
}
