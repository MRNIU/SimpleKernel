
/**
 * @file spinlock.hpp
 * @brief 自旋锁
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2022-01-01
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2022-01-01<td>MRNIU<td>迁移到 doxygen
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_INCLUDE_SPINLOCK_HPP_
#define SIMPLEKERNEL_SRC_KERNEL_INCLUDE_SPINLOCK_HPP_

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
  SpinLock(const SpinLock &) = default;
  SpinLock(SpinLock &&) = default;
  auto operator=(const SpinLock &) -> SpinLock & = default;
  auto operator=(SpinLock &&) -> SpinLock & = default;
  ~SpinLock() = default;
  /// @}

  /**
   * @brief 获得锁
   */
  void lock() {
    DisableInterruptsNested();
    if (IsLockedByCurrentCore()) {
      printf("spinlock %s IsLockedByCurrentCore == true.\n", name_);
    }
    while (locked_.test_and_set(std::memory_order_acquire)) {
      ;
    }

    std::atomic_signal_fence(std::memory_order_acquire);
    std::atomic_thread_fence(std::memory_order_acquire);

    core_id_ = core_id();
  }

  /**
   * @brief 释放锁
   */
  void unlock() {
    if (!IsLockedByCurrentCore()) {
      printf("spinlock %s IsLockedByCurrentCore == false.\n", name_);
    }
    core_id_ = SIZE_MAX;

    std::atomic_signal_fence(std::memory_order_release);
    std::atomic_thread_fence(std::memory_order_release);

    locked_.clear(std::memory_order_release);

    RestoreInterruptsNested();
  }

  //   friend std::ostream &operator<<(std::ostream &_os,
  //                                   const SpinLock &_spinlock) {
  //     printf("spinlock(%s) hart 0x%X %s\n", _spinlock.name_,
  //     _spinlock.core_id_,
  //            (_spinlock.locked_._M_i ? "locked_" : "unlock"));
  //     return _os;
  //   }

 protected:
  /// 自旋锁名称
  const char *name_{"unnamed"};
  /// 是否 lock
  std::atomic_flag locked_{ATOMIC_FLAG_INIT};
  /// 获得此锁的 core_id_
  size_t core_id_{SIZE_MAX};

  virtual void intr_on() const { cpu_io::EnableInterrupt(); }
  virtual void intr_off() const { cpu_io::DisableInterrupt(); }
  virtual auto intr_status() const -> bool {
    return cpu_io::GetInterruptStatus();
  }
  virtual auto core_id() const -> size_t { return cpu_io::GetCurrentCoreId(); }

  /**
   * @brief 检查当前 core 是否获得此锁
   * @return true             是
   * @return false            否
   */
  auto IsLockedByCurrentCore() -> bool {
    return locked_._M_i && (core_id_ == core_id());
  }

  /**
   * @brief 中断嵌套+1
   */
  void DisableInterruptsNested() {
    bool old = intr_status();

    intr_off();

    if (g_per_cpu[core_id()].noff_ == 0) {
      g_per_cpu[core_id()].intr_enable_ = old;
    }
    g_per_cpu[core_id()].noff_ += 1;
  }

  /**
   * @brief 中断嵌套-1
   */
  void RestoreInterruptsNested() {
    if (intr_status()) {
      printf("RestoreInterruptsNested - interruptible\n");
    }

    if (g_per_cpu[core_id()].noff_ < 1) {
      printf("RestoreInterruptsNested\n");
    }
    g_per_cpu[core_id()].noff_ -= 1;

    if ((g_per_cpu[core_id()].noff_ == 0) &&
        (g_per_cpu[core_id()].intr_enable_)) {
      intr_on();
    }
  }
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_SPINLOCK_HPP_ */
