/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SPINLOCK_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SPINLOCK_HPP_

#include <cpu_io.h>

#include <atomic>
#include <cstddef>

#include "sk_cstdio"

/**
 * @brief 自旋锁
 * @note 使用限制：
 * 1. 不可重入：不支持同一核心递归获取锁，会导致死锁或返回失败。
 * 2. 关中断：获取锁时会自动关闭中断，释放锁时恢复之前的状态。
 * 3. 必须配对：必须在获取锁的同一个核心释放锁。
 * 4. 不可休眠：持有自旋锁期间不可执行休眠或调度操作。
 * 5. 副作用：修改当前 CPU 的中断状态。
 */
class SpinLock {
 public:
  /**
   * @brief 构造函数
   * @param  _name            锁名
   * @note 需要堆初始化后可用
   */
  explicit SpinLock(const char* _name) : name_(_name) {}

  /// @name 构造/析构函数
  /// @{
  SpinLock() = default;
  SpinLock(const SpinLock&) = delete;
  SpinLock(SpinLock&&) = default;
  auto operator=(const SpinLock&) -> SpinLock& = delete;
  auto operator=(SpinLock&&) -> SpinLock& = default;
  ~SpinLock() = default;
  /// @}

  /**
   * @brief 获得锁
   */
  __always_inline auto lock() -> bool {
    auto intr_enable = GetInterruptStatus();
    DisableInterrupt();

    if (IsLockedByCurrentCore()) {
      sk_printf("spinlock %s IsLockedByCurrentCore == true.\n", name_);
      return false;
    }
    while (locked_.test_and_set(std::memory_order_acquire)) {
      ;
    }

    // 获取锁成功后立即设置 core_id_
    core_id_.store(GetCurrentCoreId(), std::memory_order_release);
    saved_intr_enable_ = intr_enable;
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

    // 先重置 core_id_，再释放锁
    core_id_.store(SIZE_MAX, std::memory_order_release);
    locked_.clear(std::memory_order_release);

    if (saved_intr_enable_) {
      EnableInterrupt();
    }
    return true;
  }

 protected:
  /// 自旋锁名称
  const char* name_{"unnamed"};
  /// 是否 lock
  std::atomic_flag locked_{ATOMIC_FLAG_INIT};
  /// 获得此锁的 core_id_
  std::atomic<size_t> core_id_{SIZE_MAX};
  /// 保存的中断状态
  bool saved_intr_enable_{false};

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

  /**
   * @brief 检查当前 core 是否获得此锁
   * @return true             是
   * @return false            否
   */
  __always_inline auto IsLockedByCurrentCore() -> bool {
    return locked_.test() &&
           (core_id_.load(std::memory_order_acquire) == GetCurrentCoreId());
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
  explicit LockGuard(mutex_type& mutex) : mutex_(mutex) { mutex_.lock(); }

  /**
   * @brief 析构函数，自动释放锁
   */
  ~LockGuard() { mutex_.unlock(); }

  LockGuard(const LockGuard&) = delete;
  LockGuard(LockGuard&&) = delete;
  auto operator=(const LockGuard&) -> LockGuard& = delete;
  auto operator=(LockGuard&&) -> LockGuard& = delete;

 private:
  mutex_type& mutex_;
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SPINLOCK_HPP_ */
