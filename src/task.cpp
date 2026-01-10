/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task.hpp"

#include <cpu_io.h>

#include <memory>

#include "kernel_log.hpp"
#include "scheduler/fifo_scheduler.hpp"
#include "singleton.hpp"

TaskControlBlock::TaskControlBlock(const char* name, size_t pid,
                                   ThreadEntry entry, void* arg)
    : name(name),
      pid(pid),
      trap_context_ptr(reinterpret_cast<cpu_io::TrapContext*>(
          kernel_stack_top.data() + kernel_stack_top.size() -
          sizeof(cpu_io::TrapContext))) {
  // 设置内核栈顶
  auto stack_top = reinterpret_cast<uint64_t>(kernel_stack_top.data()) +
                   kernel_stack_top.size();

  // 1. 设置 ra 指向汇编跳板 kernel_thread_entry
  // 当 switch_to 执行 ret 时，会跳转到 kernel_thread_entry
  task_context.ra = reinterpret_cast<uint64_t>(kernel_thread_entry);

  // 2. 设置 s0 保存真正的入口函数地址
  task_context.s0 = reinterpret_cast<uint64_t>(entry);

  // 3. 设置 s1 保存参数
  task_context.s1 = reinterpret_cast<uint64_t>(arg);

  // 4. 设置 sp 为栈顶
  task_context.sp = stack_top;

  // 状态设为就绪
  status = TaskStatus::kReady;
}

TaskControlBlock::TaskControlBlock(const char* name, size_t pid,
                                   ThreadEntry entry, void* arg, void* user_sp)
    : name(name),
      pid(pid),
      trap_context_ptr(reinterpret_cast<cpu_io::TrapContext*>(
          kernel_stack_top.data() + kernel_stack_top.size() -
          sizeof(cpu_io::TrapContext))) {
  // 1. 初始化 TrapContext (用户上下文)
  *trap_context_ptr = cpu_io::TrapContext();

  // sstatus: SPP=0 (User), SPIE=1 (Enable Interrupts), FS=1 (Initial)
  trap_context_ptr->sstatus = 0x00002020;
  trap_context_ptr->sepc = reinterpret_cast<uint64_t>(entry);
  trap_context_ptr->sp = reinterpret_cast<uint64_t>(user_sp);
  trap_context_ptr->a0 = reinterpret_cast<uint64_t>(arg);

  // 2. 初始化 TaskContext (内核上下文)
  // ra -> kernel_thread_entry
  // s0 -> trap_return
  // s1 -> trap_context_ptr
  task_context.ra = reinterpret_cast<uint64_t>(kernel_thread_entry);
  task_context.s0 = reinterpret_cast<uint64_t>(trap_return);
  task_context.s1 = reinterpret_cast<uint64_t>(trap_context_ptr);
  task_context.sp = reinterpret_cast<uint64_t>(trap_context_ptr);

  // 状态设为就绪
  status = TaskStatus::kReady;
}

TaskManager::TaskManager() {
  // 初始化调度器数组
  schedulers[SchedPolicy::kRealTime] = new RtScheduler();
  schedulers[SchedPolicy::kNormal] = new FifoScheduler();
  schedulers[SchedPolicy::kIdle] = nullptr;
}

void TaskManager::AddTask(TaskControlBlock* task) {
  if (task->policy < SchedPolicy::kPolicyCount) {
    schedulers[task->policy]->Enqueue(task);
  }
}

void TaskManager::Schedule() {
  TaskControlBlock* next_task = nullptr;

  // 1. 如果当前任务还在运行 (Yield的情况)，先将其放回就绪队列
  if (current_task->status == TaskStatus::kRunning) {
    current_task->status = TaskStatus::kReady;
    if (current_task->policy < SchedPolicy::kPolicyCount &&
        schedulers[current_task->policy]) {
      schedulers[current_task->policy]->Enqueue(current_task);
    }
  }

  // 2. 按优先级遍历调度器，寻找下一个任务
  // 顺序: RealTime -> Normal -> Idle (如果有)
  for (auto* sched : schedulers) {
    if (sched) {
      next_task = sched->PickNext();
      if (next_task) {
        break;
      }
    }
  }

  if (!next_task) {
    // 如果当前任务被放回去了，那它就是唯一的任务了，继续跑它
    if (current_task->status == TaskStatus::kReady) {
      current_task->status = TaskStatus::kRunning;
      return;
    }

    // 如果没有任务就绪（且当前任务也在睡眠），则等待
    // 这里简单实现为忙等待，直到有任务就绪 (由中断触发 UpdateTick 唤醒)
    while (!next_task) {
      for (auto* sched : schedulers) {
        if (sched) {
          next_task = sched->PickNext();
          if (next_task) break;
        }
      }
    }
  }

  TaskControlBlock* prev_task = current_task;
  current_task = next_task;
  current_task->status = TaskStatus::kRunning;

  // 执行上下文切换
  switch_to(&prev_task->task_context, &current_task->task_context);
}

void TaskManager::InitMainThread() {
  static TaskControlBlock main_task;  // Idle/Main thread
  main_task.pid = 0;
  main_task.status = TaskStatus::kRunning;
  main_task.policy = SchedPolicy::kNormal;  // 初始设为 Normal
  current_task = &main_task;
}

void TaskManager::UpdateTick() {
  current_tick++;
  auto it = sleeping_tasks.begin();
  while (it != sleeping_tasks.end()) {
    TaskControlBlock* task = *it;
    if (current_tick >= task->wake_tick) {
      task->status = TaskStatus::kReady;
      AddTask(task);
      it = sleeping_tasks.erase(it);
    } else {
      ++it;
    }
  }
}

void TaskManager::Sleep(uint64_t ms) {
  if (!current_task) return;

  // 至少睡眠 1 tick
  uint64_t ticks = ms * tick_frequency / 1000;
  if (ticks == 0) ticks = 1;

  current_task->wake_tick = current_tick + ticks;
  current_task->status = TaskStatus::kSleeping;
  sleeping_tasks.push_back(current_task);

  Schedule();
}
