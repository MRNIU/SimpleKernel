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
  cpu_sched.lock.lock();

  auto* current = GetCurrentTask();
  if (!current) {
    cpu_sched.lock.unlock();
    return;
  }

  // 如果睡眠时间为 0，仅让出 CPU（相当于 yield）
  if (ms == 0) {
    cpu_sched.lock.unlock();
    Schedule();
    return;
  }

  // 1. 计算唤醒时间 (当前 tick + 睡眠时间)
  // 假设 tick 频率为 1000Hz (1ms per tick)
  uint64_t sleep_ticks = ms;
  current->sched_info.wake_tick = cpu_sched.local_tick + sleep_ticks;

  // 2. 将任务标记为睡眠状态
  current->status = TaskStatus::kSleeping;

  // 3. 将任务加入睡眠队列（优先队列会自动按 wake_tick 排序）
  cpu_sched.sleeping_tasks.push(current);

  cpu_sched.lock.unlock();

  // 4. 调度到其他任务
  // 注意：Schedule() 检查到 current->status != kRunning，不会重新入队
  Schedule();

  // 5. 任务被唤醒后会从这里继续执行
  // （由 TickUpdate 在唤醒时间到达时将任务重新入队并调度）
}
