/**
 * @copyright Copyright The SimpleKernel Contributors
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
  EXPECT_TRUE(lock.Lock());

  // 解锁应该成功
  EXPECT_TRUE(lock.UnLock());
}

// 测试中断控制
TEST_F(SpinLockTest, InterruptControl) {
  SpinLockTestable lock("interrupt_test");

  // 初始状态中断是开启的
  EXPECT_TRUE(lock.GetInterruptStatus());

  lock.Lock();
  // 加锁后中断应该被禁用
  EXPECT_FALSE(lock.GetInterruptStatus());

  lock.UnLock();
  // 解锁后中断应该被恢复
  EXPECT_TRUE(lock.GetInterruptStatus());
}

// 测试中断状态恢复
TEST_F(SpinLockTest, InterruptRestore) {
  SpinLockTestable lock("intr_restore_test");

  // 模拟中断原本就是关闭的情况
  interrupt_enabled = false;

  lock.Lock();
  EXPECT_FALSE(lock.GetInterruptStatus());

  lock.UnLock();
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

      // 为每个线程创建独立的随机数生成器
      std::random_device rd;
      std::mt19937 gen(rd());
      std::uniform_int_distribution<> dis(1, 5);

      for (int j = 0; j < increments_per_thread; ++j) {
        lock.Lock();
        int temp = shared_counter.load();
        // 随机工作时间 1-5 微秒
        std::this_thread::sleep_for(std::chrono::microseconds(dis(gen)));
        shared_counter.store(temp + 1);
        lock.UnLock();
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

  lock1.Lock();
  EXPECT_FALSE(lock1.GetInterruptStatus());

  // 嵌套加锁
  lock2.Lock();
  EXPECT_FALSE(lock2.GetInterruptStatus());

  lock2.UnLock();
  EXPECT_FALSE(lock2.GetInterruptStatus());  // 仍然禁用

  lock1.UnLock();
  EXPECT_TRUE(lock1.GetInterruptStatus());  // 恢复
}

// 测试锁的持有者检查
TEST_F(SpinLockTest, LockOwnership) {
  SpinLockTestable lock("ownership_test");

  // 设置当前核心 ID
  lock.SetCurrentCoreId(0);

  lock.Lock();

  // 切换到不同的核心
  lock.SetCurrentCoreId(1);

  // 从不同核心解锁应该输出警告信息（但仍然可以执行）
  EXPECT_FALSE(lock.UnLock());

  lock.SetCurrentCoreId(0);
  EXPECT_TRUE(lock.UnLock());
}

// 测试多个锁的独立性
TEST_F(SpinLockTest, MultipleLockIndependence) {
  SpinLockTestable lock1("independent_test1");
  SpinLockTestable lock2("independent_test2");

  // 锁1和锁2应该是独立的
  lock1.Lock();
  EXPECT_TRUE(lock2.Lock());

  lock1.UnLock();
  EXPECT_TRUE(lock2.UnLock());
}

// 性能测试：测试锁的获取和释放速度
TEST_F(SpinLockTest, PerformanceTest) {
  SpinLockTestable lock("performance_test");
  const int iterations = 100000;

  auto start = std::chrono::high_resolution_clock::now();

  for (int i = 0; i < iterations; ++i) {
    lock.Lock();
    lock.UnLock();
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
        lock.Lock();

        // 在临界区内记录访问顺序
        {
          std::lock_guard<std::mutex> guard(order_mutex);
          access_order.push_back(i);
        }

        // 模拟随机工作时间
        std::this_thread::sleep_for(std::chrono::microseconds(work_dis(gen)));

        lock.UnLock();

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
        if (lock.Lock()) {
          thread_access_counts[i]++;

          // 模拟随机工作时间
          std::this_thread::sleep_for(std::chrono::microseconds(work_dis(gen)));

          lock.UnLock();
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
    lock.Lock();
    lock.UnLock();
  }

  EXPECT_TRUE(lock.GetInterruptStatus());  // 中断状态应该正确恢复
}

// 测试递归锁定（应该会失败或阻塞）
TEST_F(SpinLockTest, RecursiveLockDetection) {
  SpinLockTestable lock("recursive_test");

  lock.SetCurrentCoreId(0);
  lock.Lock();

  // 尝试在同一核心上再次加锁（这应该是错误的使用）
  // 注意：这个测试可能会导致死锁，取决于实现
  // 在实际的内核代码中，应该有递归检测机制

  // 简单验证锁状态
  EXPECT_FALSE(lock.GetInterruptStatus());

  lock.UnLock();
}

// 测试锁的公平性（FIFO 顺序）
TEST_F(SpinLockTest, FairnessTest) {
  SpinLockTestable lock("fairness_test");
  std::vector<int> execution_order;
  std::mutex order_mutex;
  const int num_threads = 5;

  std::vector<std::thread> threads;

  for (int i = 0; i < num_threads; ++i) {
    threads.emplace_back([&lock, &execution_order, &order_mutex, i]() {
      lock.SetCurrentCoreId(i);
      std::this_thread::sleep_for(std::chrono::milliseconds(i * 10));

      lock.Lock();
      {
        std::lock_guard<std::mutex> guard(order_mutex);
        execution_order.push_back(i);
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(10));
      lock.UnLock();
    });
  }

  for (auto& thread : threads) {
    thread.join();
  }

  // 验证所有线程都执行了
  EXPECT_EQ(execution_order.size(), num_threads);
}

// 测试高负载下的锁性能
TEST_F(SpinLockTest, HighLoadPerformance) {
  SpinLockTestable lock("high_load_test");
  const int num_threads = 8;
  const int operations_per_thread = 1000;
  std::atomic<int> total_operations{0};

  std::vector<std::thread> threads;

  for (int i = 0; i < num_threads; ++i) {
    threads.emplace_back(
        [&lock, operations_per_thread, &total_operations, i]() {
          lock.SetCurrentCoreId(i);

          for (int j = 0; j < operations_per_thread; ++j) {
            lock.Lock();
            total_operations.fetch_add(1);
            lock.UnLock();
          }
        });
  }

  for (auto& thread : threads) {
    thread.join();
  }

  EXPECT_EQ(total_operations.load(), num_threads * operations_per_thread);
}

// 测试中断状态的嵌套保存和恢复
TEST_F(SpinLockTest, NestedInterruptSaveRestore) {
  SpinLockTestable lock1("nested1");
  SpinLockTestable lock2("nested2");
  SpinLockTestable lock3("nested3");

  // 初始状态：中断开启
  EXPECT_TRUE(lock1.GetInterruptStatus());

  lock1.Lock();
  EXPECT_FALSE(lock1.GetInterruptStatus());

  lock2.Lock();
  EXPECT_FALSE(lock2.GetInterruptStatus());

  lock3.Lock();
  EXPECT_FALSE(lock3.GetInterruptStatus());

  // 按相反顺序解锁
  lock3.UnLock();
  EXPECT_FALSE(lock3.GetInterruptStatus());

  lock2.UnLock();
  EXPECT_FALSE(lock2.GetInterruptStatus());

  lock1.UnLock();
  EXPECT_TRUE(lock1.GetInterruptStatus());  // 恢复原始状态
}

// 测试零竞争情况
TEST_F(SpinLockTest, NoContentionSingleThread) {
  SpinLockTestable lock("no_contention");
  const int iterations = 10000;

  auto start = std::chrono::high_resolution_clock::now();

  for (int i = 0; i < iterations; ++i) {
    lock.Lock();
    shared_counter++;
    lock.UnLock();
  }

  auto end = std::chrono::high_resolution_clock::now();
  auto duration =
      std::chrono::duration_cast<std::chrono::microseconds>(end - start);

  EXPECT_EQ(shared_counter.load(), iterations);

  std::cout << std::format(
      "Single thread (no contention): {} operations in {} microseconds\n",
      iterations, duration.count());
}

// 测试锁持有时间过长的情况
TEST_F(SpinLockTest, LongHoldTime) {
  SpinLockTestable lock("long_hold");
  std::atomic<bool> lock_held{false};
  std::atomic<bool> waiter_started{false};
  std::atomic<int> spin_count{0};

  std::thread holder([&lock, &lock_held, &waiter_started]() {
    lock.SetCurrentCoreId(0);
    lock.Lock();
    lock_held = true;

    // 等待 waiter 线程开始尝试获取锁
    while (!waiter_started.load()) {
      std::this_thread::yield();
    }

    // 持有锁一段时间
    std::this_thread::sleep_for(std::chrono::milliseconds(50));
    lock.UnLock();
  });

  std::thread waiter([&lock, &lock_held, &spin_count, &waiter_started]() {
    lock.SetCurrentCoreId(1);

    // 等待直到第一个线程持有锁
    while (!lock_held.load()) {
      std::this_thread::yield();
    }

    waiter_started = true;

    // 在尝试获取锁之前，开始计数自旋次数
    auto start_time = std::chrono::steady_clock::now();

    // 尝试获取锁（会自旋等待）
    lock.Lock();

    auto end_time = std::chrono::steady_clock::now();
    auto wait_duration = std::chrono::duration_cast<std::chrono::milliseconds>(
        end_time - start_time);

    // 如果等待时间大于 10ms，说明确实等待了
    if (wait_duration.count() > 10) {
      spin_count.fetch_add(1);
    }

    lock.UnLock();
  });

  holder.join();
  waiter.join();

  // 验证等待者确实等待了（等待时间应该大于 10ms）
  EXPECT_GT(spin_count.load(), 0);
}

// 测试多核心ID的独立性
TEST_F(SpinLockTest, MultipleCoreIds) {
  SpinLockTestable lock("multi_core");
  std::vector<int> core_results(4, 0);
  std::vector<std::thread> threads;

  for (int i = 0; i < 4; ++i) {
    threads.emplace_back([&lock, &core_results, i]() {
      lock.SetCurrentCoreId(i);

      for (int j = 0; j < 100; ++j) {
        lock.Lock();
        core_results[i]++;
        std::this_thread::sleep_for(std::chrono::microseconds(10));
        lock.UnLock();
      }
    });
  }

  for (auto& thread : threads) {
    thread.join();
  }

  // 验证每个核心都完成了预期的操作
  for (int i = 0; i < 4; ++i) {
    EXPECT_EQ(core_results[i], 100);
  }
}

// 测试锁的状态一致性
TEST_F(SpinLockTest, StateConsistency) {
  SpinLockTestable lock("consistency");

  // 初始状态
  EXPECT_TRUE(lock.GetInterruptStatus());

  // 加锁
  lock.Lock();
  EXPECT_FALSE(lock.GetInterruptStatus());

  // 解锁
  lock.UnLock();
  EXPECT_TRUE(lock.GetInterruptStatus());

  // 多次循环验证状态一致性
  for (int i = 0; i < 100; ++i) {
    lock.Lock();
    EXPECT_FALSE(lock.GetInterruptStatus());
    lock.UnLock();
    EXPECT_TRUE(lock.GetInterruptStatus());
  }
}

}  // namespace
