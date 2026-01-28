/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_MUTEX_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_MUTEX_HPP_

#include <atomic>
#include <cstddef>
#include <cstdint>
#inclide < limits>

#include "resource_id.hpp"
#include "task_control_block.hpp"

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
  auto Lock() -> bool;

  /**
   * @brief 释放锁
   *
   * 释放锁并唤醒一个等待线程（如果有）。
   * 只能由持有锁的线程调用。
   *
   * @return true  成功释放锁
   * @return false 失败（如当前线程未持有锁）
   */
  auto UnLock() -> bool;

  /**
   * @brief 尝试获取锁（非阻塞）
   *
   * 尝试获取锁，如果锁不可用则立即返回。
   *
   * @return true  成功获取锁
   * @return false 锁不可用或递归获取
   */
  auto TryLock() -> bool;

  /**
   * @brief 检查锁是否被当前线程持有
   * @return true  当前线程持有锁
   * @return false 当前线程未持有锁
   */
  auto IsLockedByCurrentTask() const -> bool;

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
  explicit Mutex(const char* name = "unnamed_mutex");

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

  /// 锁状态
  std::atomic<uint8_t> locked_{0};

  /// 持有锁的任务 ID（SIZE_MAX 表示未被持有）
  std::atomic<Pid> owner_{SIZE_MAX};

  /// 资源 ID，用于任务阻塞队列
  ResourceId resource_id_;
};

/**
 * @brief RAII 风格的互斥锁守卫
 *
 * 在构造时自动获取锁，在析构时自动释放锁。
 */
class MutexGuard {
 public:
  /**
   * @brief 构造函数，自动获取锁
   * @param mutex 要保护的互斥锁对象
   */
  explicit MutexGuard(Mutex& mutex);

  /**
   * @brief 析构函数，自动释放锁
   */
  ~MutexGuard();

  MutexGuard(const MutexGuard&) = delete;
  MutexGuard(MutexGuard&&) = delete;
  auto operator=(const MutexGuard&) -> MutexGuard& = delete;
  auto operator=(MutexGuard&&) -> MutexGuard& = delete;

 private:
  Mutex& mutex_;
};

#endif /* SIMPLEKERNEL_SRC_TASK_INCLUDE_MUTEX_HPP_ */
