/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SPINLOCK_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SPINLOCK_HPP_

#include <cpu_io.h>

#include <atomic>
#include <cstddef>

#include "per_cpu.hpp"
#include "sk_cstdio"

/**
 * @brief 自旋锁
 */
class SpinLock {
 public:
  /**
   * @brief 构造函数
   * @param  _name            锁名
   * @note 需要堆初始化后可用
   */
  explicit SpinLock(const char *_name) : name_(_name) {}

  /// @name 构造/析构函数
  /// @{
  SpinLock() = default;
  SpinLock(const SpinLock &) = delete;
  SpinLock(SpinLock &&) = default;
  auto operator=(const SpinLock &) -> SpinLock & = delete;
  auto operator=(SpinLock &&) -> SpinLock & = default;
  ~SpinLock() = default;
  /// @}

  /**
   * @brief 获得锁
   */
  __always_inline auto lock() -> bool {
    if (!DisableInterruptsNested()) {
      return false;
    }
    if (IsLockedByCurrentCore()) {
      sk_printf("spinlock %s IsLockedByCurrentCore == true.\n", name_);
      return false;
    }
    while (locked_.test_and_set(std::memory_order_acquire)) {
      ;
    }

    std::atomic_thread_fence(std::memory_order_acquire);

    core_id_ = GetCurrentCoreId();
    return true;
  }

  /**
   * @brief 释放锁
   */
  __always_inline auto unlock() -> bool {
    if (!IsLockedByCurrentCore()) {
      sk_printf("spinlock %s IsLockedByCurrentCore == false.\n", name_);
      return false;
    }
    core_id_ = SIZE_MAX;

    std::atomic_thread_fence(std::memory_order_release);

    locked_.clear(std::memory_order_release);

    return RestoreInterruptsNested();
  }

 protected:
  /// 自旋锁名称
  const char *name_{"unnamed"};
  /// 是否 lock
  std::atomic_flag locked_{ATOMIC_FLAG_INIT};
  /// 获得此锁的 core_id_
  size_t core_id_{SIZE_MAX};

  virtual __always_inline void EnableInterrupt() { cpu_io::EnableInterrupt(); }
  virtual __always_inline void DisableInterrupt() {
    cpu_io::DisableInterrupt();
  }
  [[nodiscard]] virtual __always_inline auto GetInterruptStatus() -> bool {
    return cpu_io::GetInterruptStatus();
  }
  [[nodiscard]] virtual __always_inline auto GetCurrentCoreId() -> size_t {
    return cpu_io::GetCurrentCoreId();
  }
  [[nodiscard]] virtual __always_inline auto GetCurrentCore()
      -> per_cpu::PerCpu & {
    return per_cpu::GetCurrentCore();
  }

  /**
   * @brief 检查当前 core 是否获得此锁
   * @return true             是
   * @return false            否
   */
  __always_inline auto IsLockedByCurrentCore() -> bool {
    return locked_._M_i && (core_id_ == GetCurrentCoreId());
  }

  /**
   * @brief 中断嵌套+1
   */
  __always_inline auto DisableInterruptsNested() -> bool {
    bool old = GetInterruptStatus();

    DisableInterrupt();

    if (GetCurrentCore().noff_ == 0) {
      GetCurrentCore().intr_enable_ = old;
    }
    GetCurrentCore().noff_ += 1;

    return true;
  }

  /**
   * @brief 中断嵌套-1
   */
  __always_inline auto RestoreInterruptsNested() -> bool {
    if (GetInterruptStatus()) {
      sk_printf("RestoreInterruptsNested - interruptible\n");
      return false;
    }

    if (GetCurrentCore().noff_ < 1) {
      sk_printf("RestoreInterruptsNested\n");
      return false;
    }
    GetCurrentCore().noff_ -= 1;

    if ((GetCurrentCore().noff_ == 0) && (GetCurrentCore().intr_enable_)) {
      EnableInterrupt();
    }
    return true;
  }
};

/**
 * @brief RAII 风格的锁守卫模板类
 * @tparam Mutex 锁类型，必须有 lock() 和 unlock() 方法
 */
template <typename Mutex>
class LockGuard {
 public:
  using mutex_type = Mutex;

  /**
   * @brief 构造函数，自动获取锁
   * @param mutex 要保护的锁对象
   */
  explicit LockGuard(mutex_type &mutex) : mutex_(mutex) { mutex_.lock(); }

  /**
   * @brief 析构函数，自动释放锁
   */
  ~LockGuard() { mutex_.unlock(); }

  LockGuard(const LockGuard &) = delete;
  LockGuard(LockGuard &&) = delete;
  auto operator=(const LockGuard &) -> LockGuard & = delete;
  auto operator=(LockGuard &&) -> LockGuard & = delete;

 private:
  mutex_type &mutex_;
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SPINLOCK_HPP_ */
