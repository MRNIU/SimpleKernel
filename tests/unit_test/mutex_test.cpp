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

/**
 * @brief Mutex 单元测试夹具
 *
 * 单元测试主要测试 Mutex 的基本逻辑，不依赖完整的任务调度系统。
 * 对于需要任务调度的测试，应该在 system_test 中进行。
 *
 * @note 由于 Mutex 需要任务上下文，大部分实际功能测试在 system_test 中
 */
class MutexTest : public ::testing::Test {
 protected:
  void SetUp() override {}

  void TearDown() override {}
};

// 测试 Mutex 默认构造
TEST_F(MutexTest, DefaultConstruction) {
  Mutex mutex;
  EXPECT_STREQ(mutex.name_, "unnamed_mutex");
  EXPECT_FALSE(mutex.IsLockedByCurrentTask());
}

// 测试 Mutex 带名称构造
TEST_F(MutexTest, NamedConstruction) {
  Mutex mutex("test_mutex");
  EXPECT_STREQ(mutex.name_, "test_mutex");
  EXPECT_FALSE(mutex.IsLockedByCurrentTask());
}

// 测试资源 ID（通过地址推断）
TEST_F(MutexTest, ResourceIdConcept) {
  Mutex mutex1("mutex1");
  Mutex mutex2("mutex2");

  // 不同的 mutex 对象应该有不同的地址
  EXPECT_NE(&mutex1, &mutex2);

  // 验证名称是独立的
  EXPECT_STREQ(mutex1.name_, "mutex1");
  EXPECT_STREQ(mutex2.name_, "mutex2");
}

// 测试名称为公共成员变量
TEST_F(MutexTest, NameMemberAccess) {
  Mutex mutex("public_name");
  EXPECT_STREQ(mutex.name_, "public_name");

  // 可以直接访问 name_
  const char* name = mutex.name_;
  EXPECT_STREQ(name, "public_name");
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

  // 两个 mutex 应该有不同的地址
  EXPECT_NE(&mutex1, &mutex2);

  // 验证名称正确
  EXPECT_STREQ(mutex1.name_, "mutex1");
  EXPECT_STREQ(mutex2.name_, "mutex2");
}

// 测试初始状态
TEST_F(MutexTest, InitialState) {
  Mutex mutex("initial_state_test");

  // 初始状态下，锁应该是未被持有的
  EXPECT_FALSE(mutex.IsLockedByCurrentTask());

  // 验证名称
  EXPECT_STREQ(mutex.name_, "initial_state_test");
}

// 测试 Mutex 的拷贝和移动语义被禁用
TEST_F(MutexTest, CopyAndMoveSemantics) {
  // 这些应该在编译时就失败
  EXPECT_FALSE(std::is_copy_constructible_v<Mutex>);
  EXPECT_FALSE(std::is_copy_assignable_v<Mutex>);
  EXPECT_FALSE(std::is_move_constructible_v<Mutex>);
  EXPECT_FALSE(std::is_move_assignable_v<Mutex>);
}

// 测试多个 Mutex 对象的独立性
TEST_F(MutexTest, MultipleMutexIndependence) {
  Mutex mutex1("mutex1");
  Mutex mutex2("mutex2");

  // 两个 mutex 应该有不同的地址
  EXPECT_NE(&mutex1, &mutex2);

  // 两个 mutex 应该有不同的名称
  EXPECT_STRNE(mutex1.name_, mutex2.name_);
}

/**
 * @brief 测试原子操作的类型约束
 *
 * 确保内部使用的原子类型符合预期
 */
TEST_F(MutexTest, AtomicTypes) {
  // 验证 std::atomic<bool> 和 std::atomic<Pid> 是 lock-free 的
  // 这对性能很重要
  EXPECT_TRUE(std::atomic<bool>{}.is_lock_free());

  // Pid 通常是 size_t，在大多数平台上应该是 lock-free 的
  EXPECT_TRUE(std::atomic<size_t>{}.is_lock_free());
}

}  // namespace

int main(int argc, char** argv) {
  ::testing::InitGoogleTest(&argc, argv);
  return RUN_ALL_TESTS();
}
