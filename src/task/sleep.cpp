/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kernel_log.hpp"
#include "task_manager.hpp"

namespace {
/// 每秒的毫秒数
constexpr uint64_t kMillisecondsPerSecond = 1000;
}  // namespace

void TaskManager::Sleep(uint64_t ms) {
  auto& cpu_sched = GetCurrentCpuSched();

  auto* current = GetCurrentTask();

  // 如果睡眠时间为 0，仅让出 CPU（相当于 yield）
  if (ms == 0) {
    Schedule();
    return;
  }

  {
    LockGuard<SpinLock> lock_guard(cpu_sched.lock);

    if (!current) {
      klog::Err("Sleep: No current task to sleep.\n");
      return;
    }

    // 计算唤醒时间 (当前 tick + 睡眠时间)
    uint64_t sleep_ticks = (ms * SIMPLEKERNEL_TICK) / kMillisecondsPerSecond;
    current->sched_info.wake_tick = cpu_sched.local_tick + sleep_ticks;

    // 将任务标记为睡眠状态
    current->status = TaskStatus::kSleeping;

    // 将任务加入睡眠队列（优先队列会自动按 wake_tick 排序）
    cpu_sched.sleeping_tasks.push(current);
  }

  // 调度到其他任务
  Schedule();

  // 任务被唤醒后会从这里继续执行
}
