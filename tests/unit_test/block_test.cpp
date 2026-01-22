/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "resource_id.hpp"
#include "task_control_block.hpp"
#include "task_manager.hpp"

/**
 * @brief 测试 Block 相关的基本功能
 * @note 这是单元测试，主要测试阻塞状态和资源 ID
 */
class BlockTest : public ::testing::Test {
 protected:
  void SetUp() override {
    task_ = new TaskControlBlock("BlockTask", 10, nullptr, nullptr);
    task_->pid = 100;
    task_->tgid = 100;
    task_->status = TaskStatus::kRunning;
  }

  void TearDown() override { delete task_; }

  TaskControlBlock* task_ = nullptr;
};

/**
 * @brief 测试任务状态转换为阻塞
 */
TEST_F(BlockTest, TaskStatusTransitionToBlocked) {
  EXPECT_EQ(task_->status, TaskStatus::kRunning);

  // 模拟阻塞：任务应该从 kRunning 转换为 kBlocked
  task_->status = TaskStatus::kBlocked;

  EXPECT_EQ(task_->status, TaskStatus::kBlocked);
}

/**
 * @brief 测试资源 ID 的设置
 */
TEST_F(BlockTest, ResourceIdSetting) {
  // 创建一个资源 ID
  ResourceId resource_id{ResourceType::kMutex, 0x1234};

  // 设置任务阻塞在该资源上
  task_->blocked_on = resource_id;

  EXPECT_EQ(task_->blocked_on.GetType(), ResourceType::kMutex);
  EXPECT_EQ(task_->blocked_on.GetData(), 0x1234);
}

/**
 * @brief 测试不同类型的资源 ID
 */
TEST_F(BlockTest, DifferentResourceTypes) {
  // 测试 Mutex
  ResourceId mutex_id{ResourceType::kMutex, 0x1000};
  EXPECT_EQ(mutex_id.GetType(), ResourceType::kMutex);
  EXPECT_EQ(mutex_id.GetData(), 0x1000);

  // 测试 Semaphore
  ResourceId sem_id{ResourceType::kSemaphore, 0x2000};
  EXPECT_EQ(sem_id.GetType(), ResourceType::kSemaphore);
  EXPECT_EQ(sem_id.GetData(), 0x2000);

  // 测试 Condition Variable
  ResourceId cv_id{ResourceType::kCondVar, 0x3000};
  EXPECT_EQ(cv_id.GetType(), ResourceType::kCondVar);
  EXPECT_EQ(cv_id.GetData(), 0x3000);

  // 测试 IO Wait
  ResourceId io_id{ResourceType::kIoComplete, 0x4000};
  EXPECT_EQ(io_id.GetType(), ResourceType::kIoComplete);
  EXPECT_EQ(io_id.GetData(), 0x4000);

  // 测试 Pid Wait
  ResourceId pid_id{ResourceType::kChildExit, 123};
  EXPECT_EQ(pid_id.GetType(), ResourceType::kChildExit);
  EXPECT_EQ(pid_id.GetData(), 123);
}

/**
 * @brief 测试任务阻塞后清除资源 ID
 */
TEST_F(BlockTest, ClearResourceIdOnWakeup) {
  // 设置任务阻塞在某个资源上
  ResourceId resource_id{ResourceType::kMutex, 0x5678};
  task_->blocked_on = resource_id;
  task_->status = TaskStatus::kBlocked;

  EXPECT_EQ(task_->status, TaskStatus::kBlocked);
  EXPECT_EQ(task_->blocked_on.GetType(), ResourceType::kMutex);

  // 模拟唤醒：清除资源 ID，状态转换为 kReady
  task_->blocked_on = ResourceId{};
  task_->status = TaskStatus::kReady;

  EXPECT_EQ(task_->status, TaskStatus::kReady);
  EXPECT_EQ(task_->blocked_on.GetType(), ResourceType::kNone);
}

/**
 * @brief 测试资源 ID 的比较
 */
TEST_F(BlockTest, ResourceIdComparison) {
  ResourceId id1{ResourceType::kMutex, 0x1000};
  ResourceId id2{ResourceType::kMutex, 0x1000};
  ResourceId id3{ResourceType::kSemaphore, 0x1000};
  ResourceId id4{ResourceType::kMutex, 0x2000};

  // 相同的资源 ID 应该相等
  EXPECT_TRUE(id1 == id2);

  // 不同类型的资源 ID 应该不相等
  EXPECT_FALSE(id1 == id3);

  // 不同数据的资源 ID 应该不相等
  EXPECT_FALSE(id1 == id4);
}

/**
 * @brief 测试无效的资源 ID
 */
TEST_F(BlockTest, InvalidResourceId) {
  ResourceId invalid_id{};

  EXPECT_EQ(invalid_id.GetType(), ResourceType::kNone);
  EXPECT_EQ(invalid_id.GetData(), 0);
}

/**
 * @brief 测试任务初始状态下没有阻塞资源
 */
TEST_F(BlockTest, InitialTaskHasNoBlockedResource) {
  EXPECT_EQ(task_->blocked_on.GetType(), ResourceType::kNone);
  EXPECT_EQ(task_->blocked_on.GetData(), 0);
}
