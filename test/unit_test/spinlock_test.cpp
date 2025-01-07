
/**
 * @file spinlock_test.cpp
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

#include "spinlock.hpp"

#include <gmock/gmock.h>
#include <gtest/gtest.h>

#include <array>
#include <format>
#include <thread>
#include <vector>

class TestableSpinLock : public SpinLock {
 public:
  using SpinLock::SpinLock;

  // Expose protected methods for testing
  using SpinLock::DisableInterruptsNested;
  using SpinLock::IsLockedByCurrentCore;
  using SpinLock::RestoreInterruptsNested;

  void EnableInterrupt() const override {}
  void DisableInterrupt() const override {}
  auto GetInterruptStatus() const -> bool override { return false; }
  auto GetCurrentCoreId() const -> size_t override {
    return std::hash<std::thread::id>{}(std::this_thread::get_id());
  }
};

class TestableSpinLockTest : public ::testing::Test {
 public:
  TestableSpinLock spinlock{"test_spinlock"};
};

TEST_F(TestableSpinLockTest, LockUnlockTest) {
  EXPECT_FALSE(spinlock.IsLockedByCurrentCore());

  spinlock.lock();
  EXPECT_TRUE(spinlock.IsLockedByCurrentCore());

  spinlock.unlock();
  EXPECT_FALSE(spinlock.IsLockedByCurrentCore());
}

TEST_F(TestableSpinLockTest, MultiThreadLockUnlockTest) {
  auto thread_func = [this]() {
    spinlock.lock();
    EXPECT_TRUE(spinlock.IsLockedByCurrentCore());
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    spinlock.unlock();
    EXPECT_FALSE(spinlock.IsLockedByCurrentCore());

    spinlock.lock();
    EXPECT_TRUE(spinlock.IsLockedByCurrentCore());
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    spinlock.unlock();
    EXPECT_FALSE(spinlock.IsLockedByCurrentCore());
  };

  std::vector<std::thread> threads;
  for (int i = 0; i < 100; ++i) {
    threads.emplace_back(thread_func);
  }

  for (auto& thread : threads) {
    thread.detach();
  }

  std::this_thread::sleep_for(std::chrono::seconds(2));
}
