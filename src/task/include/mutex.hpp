/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_MUTEX_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_MUTEX_HPP_

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <limits>

#include "kernel_log.hpp"
#include "resource_id.hpp"
#include "singleton.hpp"
#include "task_control_block.hpp"
#include "task_manager.hpp"

/**
 * @brief 互斥锁（Mutex）
 *
 * 实现基于任务调度的互斥锁：
 * - 当锁被占用时，请求线程会被阻塞并进入等待队列
 * - 当锁释放时，会唤醒一个等待线程
 * - 支持所有者跟踪和递归检测
 *
 * @note 使用限制：
 * 1. 不可重入：同一线程不能递归获取同一锁
 * 2. 所有权：必须由获取锁的线程释放锁
 * 3. 阻塞语义：获取锁失败时会阻塞当前任务
 * 4. 必须在任务上下文中使用：不能在中断处理程序中使用
 */
class Mutex {
 public:
  /**
   * @brief 获取锁（阻塞）
   *
   * 如果锁已被其他线程持有，当前线程将被阻塞直到锁可用。
   * 如果当前线程已持有锁，返回 false（不支持递归）。
   *
   * @return true  成功获取锁
   * @return false 失败（如递归获取）
   */
  auto Lock() -> bool {
    auto current_task = Singleton<TaskManager>::GetInstance().GetCurrentTask();
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
      Singleton<TaskManager>::GetInstance().Block(resource_id_);

      // 被唤醒后重新尝试
      expected = false;
    }

    // 成功获取锁，记录所有者
    owner_.store(current_pid, std::memory_order_release);
    klog::Debug("Mutex::Lock: Task %zu acquired mutex '%s'\n", current_pid,
                name_);
    return true;
  }

  /**
   * @brief 释放锁
   *
   * 释放锁并唤醒一个等待线程（如果有）。
   * 只能由持有锁的线程调用。
   *
   * @return true  成功释放锁
   * @return false 失败（如当前线程未持有锁）
   */
  auto UnLock() -> bool {
    auto current_task = Singleton<TaskManager>::GetInstance().GetCurrentTask();
    if (current_task == nullptr) {
      klog::Err(
          "Mutex::UnLock: Cannot unlock mutex '%s' outside task context\n",
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
    owner_.store(std::numeric_limits<pid_t>::max(), std::memory_order_release);
    locked_.store(false, std::memory_order_release);

    klog::Debug("Mutex::UnLock: Task %zu released mutex '%s'\n", current_pid,
                name_);

    // 唤醒等待此锁的任务
    Singleton<TaskManager>::GetInstance().Wakeup(resource_id_);

    return true;
  }

  /**
   * @brief 尝试获取锁（非阻塞）
   *
   * 尝试获取锁，如果锁不可用则立即返回。
   *
   * @return true  成功获取锁
   * @return false 锁不可用或递归获取
   */
  auto TryLock() -> bool {
    auto current_task = Singleton<TaskManager>::GetInstance().GetCurrentTask();
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
    if (locked_.compare_exchange_strong(expected, true,
                                        std::memory_order_acquire,
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

  /**
   * @brief 检查锁是否被当前线程持有
   * @return true  当前线程持有锁
   * @return false 当前线程未持有锁
   */
  auto IsLockedByCurrentTask() const -> bool {
    auto current_task = Singleton<TaskManager>::GetInstance().GetCurrentTask();
    if (current_task == nullptr) {
      return false;
    }

    return locked_.load(std::memory_order_acquire) &&
           owner_.load(std::memory_order_acquire) == current_task->pid;
  }

  /**
   * @brief 获取资源 ID
   * @return 此互斥锁的资源 ID
   */
  auto GetResourceId() const -> ResourceId { return resource_id_; }

  /**
   * @brief 获取锁的名称
   * @return 锁的名称字符串
   */
  auto GetName() const -> const char* { return name_; }

  /**
   * @brief 构造函数
   * @param name 互斥锁名称（用于调试）
   */
  explicit Mutex(const char* name = "unnamed_mutex")
      : name_(name),
        resource_id_(ResourceType::kMutex, reinterpret_cast<uint64_t>(this)) {}

  /// @name 构造/析构函数
  /// @{
  Mutex() = default;
  Mutex(const Mutex&) = delete;
  Mutex(Mutex&&) = delete;
  auto operator=(const Mutex&) -> Mutex& = delete;
  auto operator=(Mutex&&) -> Mutex& = delete;
  ~Mutex() = default;
  /// @}

 private:
  /// 锁的名称（用于调试）
  const char* name_{"unnamed_mutex"};

  /// 锁状态：true=已锁定，false=未锁定
  std::atomic<bool> locked_{false};

  /// 持有锁的任务 ID，max() 表示未被持有
  std::atomic<Pid> owner_{std::numeric_limits<pid_t>::max()};

  /// 资源 ID，用于任务阻塞队列
  ResourceId resource_id_;
};

#endif /* SIMPLEKERNEL_SRC_TASK_INCLUDE_MUTEX_HPP_ */
