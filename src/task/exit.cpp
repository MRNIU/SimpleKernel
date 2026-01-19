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
  cpu_sched.lock.lock();

  auto* current = GetCurrentTask();
  if (!current) {
    cpu_sched.lock.unlock();
    return;
  }

  // 1. 保存退出码
  current->exit_code = exit_code;

  // 2. 检查是否有父进程
  if (current->parent_pid != 0) {
    // 有父进程，进入僵尸状态等待回收
    current->status = TaskStatus::kZombie;

    // TODO: 唤醒等待此任务的父进程（在实现 wait 系统调用后）
    // 可以使用 Wakeup(current->pid) 来唤醒等待的父进程
  } else {
    // 没有父进程（孤儿进程），直接标记为已退出
    current->status = TaskStatus::kExited;

    // TODO: 释放任务资源
    // - 释放页表
    // - 释放内核栈（需要延迟到切换后）
    // - 从任务列表中移除
  }

  cpu_sched.lock.unlock();

  // 3. 调度到其他任务
  // 注意：当前任务已标记为 kZombie 或 kExited，
  // Schedule() 不会将其重新入队
  Schedule();

  // 4. 不应该执行到这里
  // 如果执行到这里说明调度出错
  klog::Err("Exit: Task %zu should not return from Schedule()\n", current->pid);

  __builtin_unreachable();
}
