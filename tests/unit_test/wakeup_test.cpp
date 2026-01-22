/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "resource_id.hpp"
#include "task_control_block.hpp"
#include "task_manager.hpp"

/**
 * @brief 测试 Wakeup 相关的基本功能
 * @note 这是单元测试，主要测试唤醒逻辑和状态转换
 */
class WakeupTest : public ::testing::Test {
 protected:
  void SetUp() override {
    // 创建几个阻塞的任务
    task1_ = new TaskControlBlock("Blocked1", 10, nullptr, nullptr);
    task1_->pid = 100;
    task1_->tgid = 100;
    task1_->status = TaskStatus::kBlocked;

    task2_ = new TaskControlBlock("Blocked2", 10, nullptr, nullptr);
    task2_->pid = 101;
    task2_->tgid = 101;
    task2_->status = TaskStatus::kBlocked;

    task3_ = new TaskControlBlock("Blocked3", 10, nullptr, nullptr);
    task3_->pid = 102;
    task3_->tgid = 102;
    task3_->status = TaskStatus::kBlocked;

    // 设置资源 ID
    mutex_id_ = ResourceId{ResourceType::kMutex, 0x1000};
    sem_id_ = ResourceId{ResourceType::kSemaphore, 0x2000};
  }

  void TearDown() override {
    delete task1_;
    delete task2_;
    delete task3_;
  }

  TaskControlBlock* task1_ = nullptr;
  TaskControlBlock* task2_ = nullptr;
  TaskControlBlock* task3_ = nullptr;
  ResourceId mutex_id_;
  ResourceId sem_id_;
};

/**
 * @brief 测试任务从阻塞状态转换为就绪状态
 */
TEST_F(WakeupTest, BlockedToReadyTransition) {
  EXPECT_EQ(task1_->status, TaskStatus::kBlocked);

  // 唤醒任务
  task1_->status = TaskStatus::kReady;

  EXPECT_EQ(task1_->status, TaskStatus::kReady);
}

/**
 * @brief 测试清除阻塞资源 ID
 */
TEST_F(WakeupTest, ClearBlockedResourceId) {
  task1_->blocked_on = mutex_id_;

  EXPECT_EQ(task1_->blocked_on.GetType(), ResourceType::kMutex);
  EXPECT_EQ(task1_->blocked_on.GetData(), 0x1000);

  // 唤醒时清除资源 ID
  task1_->blocked_on = ResourceId{};

  EXPECT_EQ(task1_->blocked_on.GetType(), ResourceType::kNone);
  EXPECT_EQ(task1_->blocked_on.GetData(), 0);
}

/**
 * @brief 测试唤醒等待同一资源的多个任务
 */
TEST_F(WakeupTest, WakeupMultipleTasksOnSameResource) {
  // 所有任务都阻塞在同一个资源上
  task1_->blocked_on = mutex_id_;
  task2_->blocked_on = mutex_id_;
  task3_->blocked_on = mutex_id_;

  EXPECT_EQ(task1_->status, TaskStatus::kBlocked);
  EXPECT_EQ(task2_->status, TaskStatus::kBlocked);
  EXPECT_EQ(task3_->status, TaskStatus::kBlocked);

  // 唤醒所有等待该资源的任务
  task1_->status = TaskStatus::kReady;
  task1_->blocked_on = ResourceId{};

  task2_->status = TaskStatus::kReady;
  task2_->blocked_on = ResourceId{};

  task3_->status = TaskStatus::kReady;
  task3_->blocked_on = ResourceId{};

  EXPECT_EQ(task1_->status, TaskStatus::kReady);
  EXPECT_EQ(task2_->status, TaskStatus::kReady);
  EXPECT_EQ(task3_->status, TaskStatus::kReady);
}

/**
 * @brief 测试唤醒特定资源上的任务
 */
TEST_F(WakeupTest, WakeupTasksOnSpecificResource) {
  // 任务阻塞在不同的资源上
  task1_->blocked_on = mutex_id_;
  task2_->blocked_on = sem_id_;
  task3_->blocked_on = mutex_id_;

  // 只唤醒等待 mutex_id_ 的任务
  if (task1_->blocked_on == mutex_id_) {
    task1_->status = TaskStatus::kReady;
    task1_->blocked_on = ResourceId{};
  }

  if (task2_->blocked_on == mutex_id_) {
    task2_->status = TaskStatus::kReady;
    task2_->blocked_on = ResourceId{};
  }

  if (task3_->blocked_on == mutex_id_) {
    task3_->status = TaskStatus::kReady;
    task3_->blocked_on = ResourceId{};
  }

  // task1 和 task3 应该被唤醒，task2 仍然阻塞
  EXPECT_EQ(task1_->status, TaskStatus::kReady);
  EXPECT_EQ(task2_->status, TaskStatus::kBlocked);
  EXPECT_EQ(task3_->status, TaskStatus::kReady);
}

/**
 * @brief 测试唤醒后任务重新入队
 */
TEST_F(WakeupTest, RequeueAfterWakeup) {
  task1_->blocked_on = mutex_id_;
  task1_->status = TaskStatus::kBlocked;

  // 唤醒任务
  task1_->status = TaskStatus::kReady;
  task1_->blocked_on = ResourceId{};

  // 任务应该是就绪状态，可以被调度器重新入队
  EXPECT_EQ(task1_->status, TaskStatus::kReady);
  EXPECT_EQ(task1_->blocked_on.GetType(), ResourceType::kNone);
}

/**
 * @brief 测试没有任务等待资源时的唤醒
 */
TEST_F(WakeupTest, WakeupWithNoWaitingTasks) {
  // 没有任务阻塞在资源上，唤醒操作应该不做任何事
  ResourceId unused_resource{ResourceType::kCondVar, 0x3000};

  // 验证没有任务阻塞在该资源上
  EXPECT_NE(task1_->blocked_on, unused_resource);
  EXPECT_NE(task2_->blocked_on, unused_resource);
  EXPECT_NE(task3_->blocked_on, unused_resource);
}

/**
 * @brief 测试不同类型资源的唤醒
 */
TEST_F(WakeupTest, WakeupDifferentResourceTypes) {
  // Mutex
  ResourceId mutex{ResourceType::kMutex, 0x1000};
  task1_->blocked_on = mutex;
  task1_->status = TaskStatus::kBlocked;

  // Semaphore
  ResourceId sem{ResourceType::kSemaphore, 0x2000};
  task2_->blocked_on = sem;
  task2_->status = TaskStatus::kBlocked;

  // Condition Variable
  ResourceId cv{ResourceType::kCondVar, 0x3000};
  task3_->blocked_on = cv;
  task3_->status = TaskStatus::kBlocked;

  // 唤醒 Mutex
  task1_->status = TaskStatus::kReady;
  task1_->blocked_on = ResourceId{};

  // 唤醒 Semaphore
  task2_->status = TaskStatus::kReady;
  task2_->blocked_on = ResourceId{};

  // 唤醒 Condition Variable
  task3_->status = TaskStatus::kReady;
  task3_->blocked_on = ResourceId{};

  EXPECT_EQ(task1_->status, TaskStatus::kReady);
  EXPECT_EQ(task2_->status, TaskStatus::kReady);
  EXPECT_EQ(task3_->status, TaskStatus::kReady);
}

/**
 * @brief 测试 Pid Wait 类型的唤醒
 */
TEST_F(WakeupTest, WakeupPidWait) {
  Pid target_pid = 999;
  ResourceId pid_wait{ResourceType::kChildExit, target_pid};

  task1_->blocked_on = pid_wait;
  task1_->status = TaskStatus::kBlocked;

  // 当目标进程退出时，唤醒等待该 PID 的任务
  task1_->status = TaskStatus::kReady;
  task1_->blocked_on = ResourceId{};

  EXPECT_EQ(task1_->status, TaskStatus::kReady);
  EXPECT_EQ(task1_->blocked_on.GetType(), ResourceType::kNone);
}

/**
 * @brief 测试 IO Wait 类型的唤醒
 */
TEST_F(WakeupTest, WakeupIoWait) {
  ResourceId io_wait{ResourceType::kIoComplete, 0x4000};

  task1_->blocked_on = io_wait;
  task1_->status = TaskStatus::kBlocked;

  // 当 IO 操作完成时，唤醒等待的任务
  task1_->status = TaskStatus::kReady;
  task1_->blocked_on = ResourceId{};

  EXPECT_EQ(task1_->status, TaskStatus::kReady);
}
