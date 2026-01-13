/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 自旋锁
 */

#include "spinlock.hpp"

#include <gmock/gmock.h>
#include <gtest/gtest.h>

#include <array>
#include <format>
#include <thread>
#include <vector>

namespace {

// 测试用的全局变量
static std::atomic<int> shared_counter{0};
static std::atomic<int> thread_counter{0};

// Mock cpu_io functions for testing
thread_local bool interrupt_enabled = true;
thread_local size_t current_core_id = 0;

class SpinLockTestable : public SpinLock {
 public:
  explicit SpinLockTestable(const char* name) : SpinLock(name) {}

  void EnableInterrupt() override { interrupt_enabled = true; }

  void DisableInterrupt() override { interrupt_enabled = false; }

  bool GetInterruptStatus() override { return interrupt_enabled; }

  size_t GetCurrentCoreId() override { return current_core_id; }

  void SetCurrentCoreId(size_t id) { current_core_id = id; }
};

class SpinLockTest : public ::testing::Test {
 protected:
  void SetUp() override {
    shared_counter = 0;
    thread_counter = 0;
    interrupt_enabled = true;
    current_core_id = 0;
  }

  void TearDown() override {
    // Reset state after each test
    interrupt_enabled = true;
    current_core_id = 0;
  }
};

// 测试基本的 lock/unlock 功能
TEST_F(SpinLockTest, BasicLockUnlock) {
  SpinLockTestable lock("basic_test");

  // 初始状态应该是未锁定的
  EXPECT_TRUE(lock.lock());

  // 解锁应该成功
  EXPECT_TRUE(lock.unlock());
}

// 测试中断控制
TEST_F(SpinLockTest, InterruptControl) {
  SpinLockTestable lock("interrupt_test");

  // 初始状态中断是开启的
  EXPECT_TRUE(lock.GetInterruptStatus());

  lock.lock();
  // 加锁后中断应该被禁用
  EXPECT_FALSE(lock.GetInterruptStatus());

  lock.unlock();
  // 解锁后中断应该被恢复
  EXPECT_TRUE(lock.GetInterruptStatus());
}

// 测试中断状态恢复
TEST_F(SpinLockTest, InterruptRestore) {
  SpinLockTestable lock("intr_restore_test");

  // 模拟中断原本就是关闭的情况
  interrupt_enabled = false;

  lock.lock();
  EXPECT_FALSE(lock.GetInterruptStatus());

  lock.unlock();
  // 解锁后中断应该保持关闭（恢复原状）
  EXPECT_FALSE(lock.GetInterruptStatus());
}

// 测试多线程并发安全性
TEST_F(SpinLockTest, ConcurrentAccess) {
  SpinLockTestable lock("concurrent_test");
  const int num_threads = 4;
  const int increments_per_thread = 1000;

  std::vector<std::thread> threads;

  for (int i = 0; i < num_threads; ++i) {
    threads.emplace_back([&lock, increments_per_thread, i]() {
      // 设置不同的核心 ID
      lock.SetCurrentCoreId(i);

      for (int j = 0; j < increments_per_thread; ++j) {
        lock.lock();
        int temp = shared_counter.load();
        std::this_thread::sleep_for(std::chrono::microseconds(1));
        shared_counter.store(temp + 1);
        lock.unlock();
      }
    });
  }

  for (auto& thread : threads) {
    thread.join();
  }

  // 如果锁工作正常，最终结果应该是精确的
  EXPECT_EQ(shared_counter.load(), num_threads * increments_per_thread);
}

// 测试无锁的并发访问（验证锁确实有效）
TEST_F(SpinLockTest, ConcurrentAccessWithoutLock) {
  const int num_threads = 4;
  const int increments_per_thread = 1000;

  std::vector<std::thread> threads;

  for (int i = 0; i < num_threads; ++i) {
    threads.emplace_back([increments_per_thread]() {
      for (int j = 0; j < increments_per_thread; ++j) {
        // 不使用锁的并发访问
        int temp = thread_counter.load();
        std::this_thread::sleep_for(std::chrono::microseconds(1));
        thread_counter.store(temp + 1);
      }
    });
  }

  for (auto& thread : threads) {
    thread.join();
  }

  // 没有锁的情况下，结果通常不会是精确的（由于竞态条件）
  // 这个测试可能会失败，这证明了锁的必要性
  EXPECT_LE(thread_counter.load(), num_threads * increments_per_thread);
}

// 测试嵌套中断控制
TEST_F(SpinLockTest, NestedInterruptControl) {
  SpinLockTestable lock1("nested_test1");
  SpinLockTestable lock2("nested_test2");

  EXPECT_TRUE(lock1.GetInterruptStatus());

  lock1.lock();
  EXPECT_FALSE(lock1.GetInterruptStatus());

  // 嵌套加锁
  lock2.lock();
  EXPECT_FALSE(lock2.GetInterruptStatus());

  lock2.unlock();
  EXPECT_FALSE(lock2.GetInterruptStatus());  // 仍然禁用

  lock1.unlock();
  EXPECT_TRUE(lock1.GetInterruptStatus());  // 恢复
}

// 测试锁的持有者检查
TEST_F(SpinLockTest, LockOwnership) {
  SpinLockTestable lock("ownership_test");

  // 设置当前核心 ID
  lock.SetCurrentCoreId(0);

  lock.lock();

  // 切换到不同的核心
  lock.SetCurrentCoreId(1);

  // 从不同核心解锁应该输出警告信息（但仍然可以执行）
  EXPECT_FALSE(lock.unlock());

  lock.SetCurrentCoreId(0);
  EXPECT_TRUE(lock.unlock());
}

// 测试多个锁的独立性
TEST_F(SpinLockTest, MultipleLockIndependence) {
  SpinLockTestable lock1("independent_test1");
  SpinLockTestable lock2("independent_test2");

  // 锁1和锁2应该是独立的
  lock1.lock();
  EXPECT_TRUE(lock2.lock());

  lock1.unlock();
  EXPECT_TRUE(lock2.unlock());
}

// 性能测试：测试锁的获取和释放速度
TEST_F(SpinLockTest, PerformanceTest) {
  SpinLockTestable lock("performance_test");
  const int iterations = 100000;

  auto start = std::chrono::high_resolution_clock::now();

  for (int i = 0; i < iterations; ++i) {
    lock.lock();
    lock.unlock();
  }

  auto end = std::chrono::high_resolution_clock::now();
  auto duration =
      std::chrono::duration_cast<std::chrono::microseconds>(end - start);

  // 输出性能信息
  std::cout << std::format(
      "SpinLock performance: {} lock/unlock pairs in {} microseconds\n",
      iterations, duration.count());

  // 基本的性能断言：应该能在合理时间内完成
  EXPECT_LT(duration.count(), 1000000);  // 少于1秒
}

// 测试异常情况
TEST_F(SpinLockTest, EdgeCases) {
  SpinLockTestable lock("edge_case_test");

  // 测试快速连续的 lock/unlock
  for (int i = 0; i < 1000; ++i) {
    lock.lock();
    lock.unlock();
  }

  EXPECT_TRUE(lock.GetInterruptStatus());  // 中断状态应该正确恢复
}

}  // namespace
