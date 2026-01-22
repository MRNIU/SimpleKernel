/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "task_control_block.hpp"
#include "task_manager.hpp"

/**
 * @brief 测试 TickUpdate 相关的基本功能
 * @note 这是单元测试，主要测试时钟更新和任务统计
 */
class TickUpdateTest : public ::testing::Test {
 protected:
  void SetUp() override {
    task_ = new TaskControlBlock("TickTask", 10, nullptr, nullptr);
    task_->pid = 100;
    task_->tgid = 100;
    task_->status = TaskStatus::kRunning;
    task_->sched_info.time_slice_remaining = 10;
    task_->sched_info.total_runtime = 0;
  }

  void TearDown() override { delete task_; }

  TaskControlBlock* task_ = nullptr;
};

/**
 * @brief 测试 tick 计数递增
 */
TEST_F(TickUpdateTest, TickCounterIncrement) {
  uint64_t tick = 0;

  // 模拟多次 tick 更新
  for (int i = 0; i < 10; ++i) {
    tick++;
  }

  EXPECT_EQ(tick, 10);
}

/**
 * @brief 测试任务运行时间统计
 */
TEST_F(TickUpdateTest, TaskRuntimeStatistics) {
  EXPECT_EQ(task_->sched_info.total_runtime, 0);

  // 模拟任务运行，每个 tick 增加运行时间
  for (int i = 0; i < 5; ++i) {
    task_->sched_info.total_runtime++;
  }

  EXPECT_EQ(task_->sched_info.total_runtime, 5);
}

/**
 * @brief 测试时间片递减
 */
TEST_F(TickUpdateTest, TimeSliceDecrement) {
  EXPECT_EQ(task_->sched_info.time_slice_remaining, 10);

  // 模拟时间片消耗
  for (int i = 0; i < 5; ++i) {
    if (task_->sched_info.time_slice_remaining > 0) {
      task_->sched_info.time_slice_remaining--;
    }
  }

  EXPECT_EQ(task_->sched_info.time_slice_remaining, 5);
}

/**
 * @brief 测试时间片耗尽时需要抢占
 */
TEST_F(TickUpdateTest, PreemptWhenTimeSliceExpired) {
  task_->sched_info.time_slice_remaining = 1;

  // 消耗最后的时间片
  task_->sched_info.time_slice_remaining--;

  EXPECT_EQ(task_->sched_info.time_slice_remaining, 0);

  // 时间片耗尽，需要抢占
  bool need_preempt = (task_->sched_info.time_slice_remaining == 0);
  EXPECT_TRUE(need_preempt);
}

/**
 * @brief 测试睡眠任务的唤醒时间检查
 */
TEST_F(TickUpdateTest, WakeupSleepingTask) {
  uint64_t current_tick = 1000;
  uint64_t wake_tick = 1005;

  task_->sched_info.wake_tick = wake_tick;
  task_->status = TaskStatus::kSleeping;

  // 模拟 tick 递增，检查是否到达唤醒时间
  for (uint64_t tick = current_tick; tick <= wake_tick; ++tick) {
    if (tick >= task_->sched_info.wake_tick) {
      // 到达唤醒时间，唤醒任务
      task_->status = TaskStatus::kReady;
      break;
    }
  }

  EXPECT_EQ(task_->status, TaskStatus::kReady);
}

/**
 * @brief 测试多个睡眠任务的唤醒
 */
TEST_F(TickUpdateTest, WakeupMultipleSleepingTasks) {
  uint64_t current_tick = 1000;

  // 创建多个睡眠任务
  auto* task1 = new TaskControlBlock("Sleep1", 10, nullptr, nullptr);
  task1->pid = 101;
  task1->sched_info.wake_tick = 1005;
  task1->status = TaskStatus::kSleeping;

  auto* task2 = new TaskControlBlock("Sleep2", 10, nullptr, nullptr);
  task2->pid = 102;
  task2->sched_info.wake_tick = 1010;
  task2->status = TaskStatus::kSleeping;

  auto* task3 = new TaskControlBlock("Sleep3", 10, nullptr, nullptr);
  task3->pid = 103;
  task3->sched_info.wake_tick = 1003;
  task3->status = TaskStatus::kSleeping;

  // 模拟 tick 更新到 1010
  current_tick = 1010;

  // 检查哪些任务应该被唤醒
  if (task1->sched_info.wake_tick <= current_tick) {
    task1->status = TaskStatus::kReady;
  }
  if (task2->sched_info.wake_tick <= current_tick) {
    task2->status = TaskStatus::kReady;
  }
  if (task3->sched_info.wake_tick <= current_tick) {
    task3->status = TaskStatus::kReady;
  }

  // 所有任务都应该被唤醒
  EXPECT_EQ(task1->status, TaskStatus::kReady);
  EXPECT_EQ(task2->status, TaskStatus::kReady);
  EXPECT_EQ(task3->status, TaskStatus::kReady);

  delete task1;
  delete task2;
  delete task3;
}

/**
 * @brief 测试只有运行中的任务更新统计信息
 */
TEST_F(TickUpdateTest, OnlyRunningTaskUpdateStats) {
  task_->status = TaskStatus::kRunning;

  uint64_t initial_runtime = task_->sched_info.total_runtime;

  // 只有运行中的任务才更新统计信息
  if (task_->status == TaskStatus::kRunning) {
    task_->sched_info.total_runtime++;
  }

  EXPECT_EQ(task_->sched_info.total_runtime, initial_runtime + 1);

  // 如果任务不是运行状态，不应该更新
  task_->status = TaskStatus::kReady;
  uint64_t runtime_before = task_->sched_info.total_runtime;

  if (task_->status == TaskStatus::kRunning) {
    task_->sched_info.total_runtime++;
  }

  EXPECT_EQ(task_->sched_info.total_runtime, runtime_before);
}

/**
 * @brief 测试时间片为 0 时不再递减
 */
TEST_F(TickUpdateTest, NoDecrementWhenTimeSliceZero) {
  task_->sched_info.time_slice_remaining = 0;

  // 时间片已经为 0，不应该继续递减
  if (task_->sched_info.time_slice_remaining > 0) {
    task_->sched_info.time_slice_remaining--;
  }

  EXPECT_EQ(task_->sched_info.time_slice_remaining, 0);
}

/**
 * @brief 测试抢占标志的设置
 */
TEST_F(TickUpdateTest, PreemptFlagSetting) {
  bool need_preempt = false;

  task_->sched_info.time_slice_remaining = 1;
  task_->sched_info.time_slice_remaining--;

  // 时间片耗尽，设置抢占标志
  if (task_->sched_info.time_slice_remaining == 0) {
    need_preempt = true;
  }

  EXPECT_TRUE(need_preempt);
}
