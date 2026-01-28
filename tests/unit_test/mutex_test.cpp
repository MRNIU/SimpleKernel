/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "mutex.hpp"

#include <gmock/gmock.h>
#include <gtest/gtest.h>

#include <array>
#include <atomic>
#include <thread>
#include <vector>

#include "singleton.hpp"
#include "task_control_block.hpp"
#include "task_manager.hpp"

namespace {

// Mock 全局变量
static std::atomic<int> shared_counter{0};

/**
 * @brief Mutex 单元测试夹具
 *
 * 单元测试主要测试 Mutex 的基本逻辑，不依赖完整的任务调度系统。
 * 对于需要任务调度的测试，应该在 system_test 中进行。
 */
class MutexTest : public ::testing::Test {
 protected:
  void SetUp() override { shared_counter = 0; }

  void TearDown() override {}
};

// 测试 Mutex 构造
TEST_F(MutexTest, Construction) {
  Mutex mutex1;
  EXPECT_STREQ(mutex1.GetName(), "unnamed_mutex");

  Mutex mutex2("test_mutex");
  EXPECT_STREQ(mutex2.GetName(), "test_mutex");
}

// 测试资源 ID 生成
TEST_F(MutexTest, ResourceId) {
  Mutex mutex("resource_test");
  auto resource_id = mutex.GetResourceId();

  // 检查资源类型
  EXPECT_EQ(resource_id.GetType(), ResourceType::kMutex);

  // 资源数据应该是 mutex 对象的地址
  EXPECT_EQ(resource_id.GetData(), reinterpret_cast<uint64_t>(&mutex));
}

// 测试基本的 TryLock 功能（不需要任务上下文）
TEST_F(MutexTest, TryLockWithoutTaskContext) {
  Mutex mutex("trylock_test");

  // 在没有任务上下文时，TryLock 应该返回 false
  EXPECT_FALSE(mutex.TryLock());
  EXPECT_FALSE(mutex.IsLockedByCurrentTask());
}

// 测试基本的 Lock 功能（不需要任务上下文）
TEST_F(MutexTest, LockWithoutTaskContext) {
  Mutex mutex("lock_test");

  // 在没有任务上下文时，Lock 应该返回 false
  EXPECT_FALSE(mutex.Lock());
  EXPECT_FALSE(mutex.IsLockedByCurrentTask());
}

// 测试 UnLock（不需要任务上下文）
TEST_F(MutexTest, UnLockWithoutTaskContext) {
  Mutex mutex("unlock_test");

  // 在没有任务上下文时，UnLock 应该返回 false
  EXPECT_FALSE(mutex.UnLock());
}

// 测试多个 Mutex 对象的独立性
TEST_F(MutexTest, MultipleMutexes) {
  Mutex mutex1("mutex1");
  Mutex mutex2("mutex2");

  // 两个 mutex 应该有不同的资源 ID
  EXPECT_NE(mutex1.GetResourceId(), mutex2.GetResourceId());

  // 检查资源 ID 中的数据部分
  auto rid1 = mutex1.GetResourceId();
  auto rid2 = mutex2.GetResourceId();
  EXPECT_EQ(rid1.GetData(), reinterpret_cast<uint64_t>(&mutex1));
  EXPECT_EQ(rid2.GetData(), reinterpret_cast<uint64_t>(&mutex2));
}

// 测试原子操作的基本功能
TEST_F(MutexTest, AtomicOperations) {
  Mutex mutex("atomic_test");

  // 初始状态下，锁应该是未被持有的
  EXPECT_FALSE(mutex.IsLockedByCurrentTask());

  // 测试资源 ID 的类型
  auto rid = mutex.GetResourceId();
  EXPECT_EQ(rid.GetType(), ResourceType::kMutex);
  EXPECT_STREQ(rid.GetTypeName(), "Mutex");
}

/**
 * @brief 测试并发场景下的计数器正确性
 *
 * 虽然不能在单元测试中完全测试任务调度，但可以测试
 * 在多线程环境下原子操作的正确性。
 */
TEST_F(MutexTest, ThreadSafety) {
  constexpr int kNumThreads = 4;
  constexpr int kIterations = 1000;

  std::atomic<int> counter{0};

  auto worker = [&counter]() {
    for (int i = 0; i < kIterations; ++i) {
      counter.fetch_add(1, std::memory_order_relaxed);
    }
  };

  std::vector<std::thread> threads;
  for (int i = 0; i < kNumThreads; ++i) {
    threads.emplace_back(worker);
  }

  for (auto& t : threads) {
    t.join();
  }

  // 验证最终计数
  EXPECT_EQ(counter.load(), kNumThreads * kIterations);
}

}  // namespace

int main(int argc, char** argv) {
  ::testing::InitGoogleTest(&argc, argv);
  return RUN_ALL_TESTS();
}
