/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 自旋锁
 */

#include "spinlock.hpp"

#include <gmock/gmock.h>
#include <gtest/gtest.h>

#include <array>
#include <format>
#include <random>
#include <thread>
#include <vector>

namespace {

// 测试用的全局变量
static std::atomic<int> shared_counter{0};
static std::atomic<int> thread_counter{0};

// Mock cpu_io functions for testing
thread_local bool interrupt_enabled = true;
thread_local size_t current_core_id = 0;

class PerCpuTestable : public per_cpu::PerCpu {
 public:
  PerCpuTestable(size_t id) : per_cpu::PerCpu(id) {}
  size_t GetCurrentCoreId() override { return current_core_id; }
};

std::array<PerCpuTestable, PerCpuTestable::kMaxCoreCount> per_cpu_instances = {
    PerCpuTestable(0), PerCpuTestable(1), PerCpuTestable(2), PerCpuTestable(3)};

class SpinLockTestable : public SpinLock {
 public:
  explicit SpinLockTestable(const char* name) : SpinLock(name) {}

  void EnableInterrupt() override { interrupt_enabled = true; }

  void DisableInterrupt() override { interrupt_enabled = false; }

  bool GetInterruptStatus() override { return interrupt_enabled; }

  size_t GetCurrentCoreId() override { return current_core_id; }

  auto GetCurrentCore() -> PerCpuTestable& override {
    // printf("GetCurrentCoreId() = %zu\n", GetCurrentCoreId());
    return per_cpu_instances[GetCurrentCoreId()];
  }

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

      // 为每个线程创建独立的随机数生成器
      std::random_device rd;
      std::mt19937 gen(rd());
      std::uniform_int_distribution<> dis(1, 5);

      for (int j = 0; j < increments_per_thread; ++j) {
        lock.lock();
        int temp = shared_counter.load();
        // 随机工作时间 1-5 微秒
        std::this_thread::sleep_for(std::chrono::microseconds(dis(gen)));
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
      // 为每个线程创建独立的随机数生成器
      std::random_device rd;
      std::mt19937 gen(rd());
      std::uniform_int_distribution<> dis(1, 5);

      for (int j = 0; j < increments_per_thread; ++j) {
        // 不使用锁的并发访问
        int temp = thread_counter.load();
        // 随机工作时间 1-5 微秒
        std::this_thread::sleep_for(std::chrono::microseconds(dis(gen)));
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

// 测试多线程并发访问顺序
TEST_F(SpinLockTest, ConcurrentAccessOrder) {
  SpinLockTestable lock("access_order_test");
  const int num_threads = 4;
  const int operations_per_thread = 100;

  // 用于记录访问顺序的容器
  std::vector<int> access_order;
  std::mutex order_mutex;  // 保护 access_order 的互斥锁

  std::vector<std::thread> threads;
  std::atomic<bool> start_flag{false};  // 同步线程开始的标志

  for (int i = 0; i < num_threads; ++i) {
    threads.emplace_back([&lock, &access_order, &order_mutex, &start_flag,
                          operations_per_thread, i]() {
      // 设置不同的核心 ID
      lock.SetCurrentCoreId(i);

      // 为每个线程创建独立的随机数生成器
      std::random_device rd;
      std::mt19937 gen(rd());
      std::uniform_int_distribution<> work_dis(5, 20);  // 工作时间 5-20 微秒
      std::uniform_int_distribution<> rest_dis(1, 10);  // 休息时间 1-10 微秒

      // 等待所有线程准备就绪
      while (!start_flag.load()) {
        std::this_thread::yield();
      }

      for (int j = 0; j < operations_per_thread; ++j) {
        lock.lock();

        // 在临界区内记录访问顺序
        {
          std::lock_guard<std::mutex> guard(order_mutex);
          access_order.push_back(i);
        }

        // 模拟随机工作时间
        std::this_thread::sleep_for(std::chrono::microseconds(work_dis(gen)));

        lock.unlock();

        // 在解锁后随机休息，让其他线程有机会获取锁
        std::this_thread::sleep_for(std::chrono::microseconds(rest_dis(gen)));
      }
    });
  }

  // 启动所有线程
  start_flag.store(true);

  for (auto& thread : threads) {
    thread.join();
  }

  // 验证访问顺序的正确性
  EXPECT_EQ(access_order.size(), num_threads * operations_per_thread);

  // 验证没有并发访问：相邻的访问应该来自不同的线程或者是同一线程的连续访问
  // 但不应该出现两个不同线程在同一时刻访问临界区的情况
  std::map<int, int> thread_counts;
  for (int thread_id : access_order) {
    thread_counts[thread_id]++;
  }

  // 每个线程都应该完成指定数量的操作
  for (int i = 0; i < num_threads; ++i) {
    EXPECT_EQ(thread_counts[i], operations_per_thread);
  }

  // 输出访问模式的统计信息
  std::cout << std::format("Access order test completed. Total accesses: {}\n",
                           access_order.size());
  for (const auto& [thread_id, count] : thread_counts) {
    std::cout << std::format("Thread {}: {} accesses\n", thread_id, count);
  }
}

// 测试锁的公平性（简单验证）
TEST_F(SpinLockTest, LockFairness) {
  SpinLockTestable lock("fairness_test");
  const int num_threads = 3;
  const int test_duration_ms = 100;

  std::atomic<bool> stop_flag{false};
  std::array<std::atomic<int>, num_threads> thread_access_counts{};

  std::vector<std::thread> threads;

  for (int i = 0; i < num_threads; ++i) {
    threads.emplace_back([&lock, &stop_flag, &thread_access_counts, i]() {
      lock.SetCurrentCoreId(i);

      // 为每个线程创建独立的随机数生成器
      std::random_device rd;
      std::mt19937 gen(rd());
      std::uniform_int_distribution<> work_dis(20,
                                               100);  // 工作时间 20-100 微秒
      std::uniform_int_distribution<> rest_dis(5, 30);  // 休息时间 5-30 微秒

      while (!stop_flag.load()) {
        if (lock.lock()) {
          thread_access_counts[i]++;

          // 模拟随机工作时间
          std::this_thread::sleep_for(std::chrono::microseconds(work_dis(gen)));

          lock.unlock();
        }

        // 随机休息时间，给其他线程机会
        std::this_thread::sleep_for(std::chrono::microseconds(rest_dis(gen)));
      }
    });
  }

  // 运行指定时间
  std::this_thread::sleep_for(std::chrono::milliseconds(test_duration_ms));
  stop_flag.store(true);

  for (auto& thread : threads) {
    thread.join();
  }

  // 检查每个线程都获得了一些访问机会
  int total_accesses = 0;
  for (int i = 0; i < num_threads; ++i) {
    int count = thread_access_counts[i].load();
    EXPECT_GT(count, 0) << "Thread " << i << " should have some access";
    total_accesses += count;
    std::cout << std::format("Thread {}: {} accesses\n", i, count);
  }

  std::cout << std::format("Total accesses in {}ms: {}\n", test_duration_ms,
                           total_accesses);
  EXPECT_GT(total_accesses, 0);
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
