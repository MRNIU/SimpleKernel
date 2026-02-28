/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "mutex.hpp"

#include "kernel.h"
#include "kernel_log.hpp"
#include "task_manager.hpp"

auto Mutex::Lock() -> bool {
  auto current_task = TaskManagerSingleton::instance().GetCurrentTask();
  if (current_task == nullptr) {
    klog::Err("Mutex::Lock: Cannot lock mutex '%s' outside task context\n",
              name_);
    return false;
  }

  Pid current_pid = current_task->pid;

  // 检查是否递归获取锁
  if (IsLockedByCurrentTask()) {
    klog::Warn("Mutex::Lock: Task %zu tried to recursively lock mutex '%s'\n",
               current_pid, name_);
    return false;
  }

  // 尝试获取锁
  bool expected = false;
  while (!locked_.compare_exchange_weak(
      expected, true, std::memory_order_acquire, std::memory_order_relaxed)) {
    // 锁被占用，阻塞当前任务
    klog::Debug("Mutex::Lock: Task %zu blocking on mutex '%s'\n", current_pid,
                name_);
    TaskManagerSingleton::instance().Block(resource_id_);

    // 被唤醒后重新尝试
    expected = false;
  }

  // 成功获取锁，记录所有者
  owner_.store(current_pid, std::memory_order_release);
  klog::Debug("Mutex::Lock: Task %zu acquired mutex '%s'\n", current_pid,
              name_);
  return true;
}

auto Mutex::UnLock() -> bool {
  auto current_task = TaskManagerSingleton::instance().GetCurrentTask();
  if (current_task == nullptr) {
    klog::Err("Mutex::UnLock: Cannot unlock mutex '%s' outside task context\n",
              name_);
    return false;
  }

  Pid current_pid = current_task->pid;

  // 检查是否由持有锁的任务释放
  if (!IsLockedByCurrentTask()) {
    klog::Warn(
        "Mutex::UnLock: Task %zu tried to unlock mutex '%s' it doesn't own\n",
        current_pid, name_);
    return false;
  }

  // 释放锁
  owner_.store(std::numeric_limits<Pid>::max(), std::memory_order_release);
  locked_.store(false, std::memory_order_release);

  klog::Debug("Mutex::UnLock: Task %zu released mutex '%s'\n", current_pid,
              name_);

  // 唤醒等待此锁的任务
  TaskManagerSingleton::instance().Wakeup(resource_id_);

  return true;
}

auto Mutex::TryLock() -> bool {
  auto current_task = TaskManagerSingleton::instance().GetCurrentTask();
  if (current_task == nullptr) {
    klog::Err(
        "Mutex::TryLock: Cannot trylock mutex '%s' outside task context\n",
        name_);
    return false;
  }

  Pid current_pid = current_task->pid;

  // 检查是否递归获取锁
  if (IsLockedByCurrentTask()) {
    klog::Debug(
        "Mutex::TryLock: Task %zu tried to recursively trylock mutex '%s'\n",
        current_pid, name_);
    return false;
  }

  // 尝试获取锁（非阻塞）
  bool expected = false;
  if (locked_.compare_exchange_strong(expected, true, std::memory_order_acquire,
                                      std::memory_order_relaxed)) {
    // 成功获取锁，记录所有者
    owner_.store(current_pid, std::memory_order_release);
    klog::Debug("Mutex::TryLock: Task %zu acquired mutex '%s'\n", current_pid,
                name_);
    return true;
  }

  // 锁被占用
  klog::Debug("Mutex::TryLock: Task %zu failed to acquire mutex '%s'\n",
              current_pid, name_);
  return false;
}

auto Mutex::IsLockedByCurrentTask() const -> bool {
  auto current_task = TaskManagerSingleton::instance().GetCurrentTask();
  if (current_task == nullptr) {
    return false;
  }

  return locked_.load(std::memory_order_acquire) &&
         owner_.load(std::memory_order_acquire) == current_task->pid;
}
