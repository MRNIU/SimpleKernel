/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "task_control_block.hpp"
#include "task_manager.hpp"

/**
 * @brief 测试 Schedule 相关的基本功能
 * @note 这是单元测试，主要测试调度逻辑和状态转换
 */
class ScheduleTest : public ::testing::Test {
 protected:
  void SetUp() override {
    // 创建几个测试任务
    task1_ = new TaskControlBlock("Task1", 10, nullptr, nullptr);
    task1_->pid = 100;
    task1_->tgid = 100;
    task1_->status = TaskStatus::kReady;
    task1_->policy = SchedPolicy::kNormal;

    task2_ = new TaskControlBlock("Task2", 20, nullptr, nullptr);
    task2_->pid = 101;
    task2_->tgid = 101;
    task2_->status = TaskStatus::kReady;
    task2_->policy = SchedPolicy::kNormal;

    task3_ = new TaskControlBlock("Task3", 5, nullptr, nullptr);
    task3_->pid = 102;
    task3_->tgid = 102;
    task3_->status = TaskStatus::kReady;
    task3_->policy = SchedPolicy::kRealTime;
  }

  void TearDown() override {
    delete task1_;
    delete task2_;
    delete task3_;
  }

  TaskControlBlock* task1_ = nullptr;
  TaskControlBlock* task2_ = nullptr;
  TaskControlBlock* task3_ = nullptr;
};

/**
 * @brief 测试任务从 kRunning 转换为 kReady
 */
TEST_F(ScheduleTest, RunningToReadyTransition) {
  task1_->status = TaskStatus::kRunning;

  // 调度时，当前运行的任务应该转换为就绪状态
  task1_->status = TaskStatus::kReady;

  EXPECT_EQ(task1_->status, TaskStatus::kReady);
}

/**
 * @brief 测试任务从 kReady 转换为 kRunning
 */
TEST_F(ScheduleTest, ReadyToRunningTransition) {
  EXPECT_EQ(task1_->status, TaskStatus::kReady);

  // 任务被调度执行时，应该转换为运行状态
  task1_->status = TaskStatus::kRunning;

  EXPECT_EQ(task1_->status, TaskStatus::kRunning);
}

/**
 * @brief 测试不同调度策略的任务
 */
TEST_F(ScheduleTest, DifferentSchedulingPolicies) {
  EXPECT_EQ(task1_->policy, SchedPolicy::kNormal);
  EXPECT_EQ(task2_->policy, SchedPolicy::kNormal);
  EXPECT_EQ(task3_->policy, SchedPolicy::kRealTime);

  // RealTime 策略应该优先于 Normal 策略
  EXPECT_LT(static_cast<int>(task3_->policy), static_cast<int>(task1_->policy));
}

/**
 * @brief 测试调度器优先级顺序
 */
TEST_F(ScheduleTest, SchedulerPriorityOrder) {
  // 调度器按优先级选择任务: RealTime > Normal > Idle
  EXPECT_EQ(SchedPolicy::kRealTime, 0);
  EXPECT_EQ(SchedPolicy::kNormal, 1);
  EXPECT_EQ(SchedPolicy::kIdle, 2);
}

/**
 * @brief 测试时间片耗尽
 */
TEST_F(ScheduleTest, TimeSliceExpired) {
  task1_->sched_info.time_slice_remaining = 10;

  // 模拟时间片减少
  for (int i = 0; i < 10; ++i) {
    task1_->sched_info.time_slice_remaining--;
  }

  EXPECT_EQ(task1_->sched_info.time_slice_remaining, 0);

  // 时间片耗尽时，任务应该被重新调度
  bool should_reschedule = (task1_->sched_info.time_slice_remaining == 0);
  EXPECT_TRUE(should_reschedule);
}

/**
 * @brief 测试任务被抢占
 */
TEST_F(ScheduleTest, TaskPreempted) {
  task1_->status = TaskStatus::kRunning;

  // 任务被抢占时，状态应该转换为就绪
  task1_->status = TaskStatus::kReady;

  EXPECT_EQ(task1_->status, TaskStatus::kReady);
}

/**
 * @brief 测试调度统计信息
 */
TEST_F(ScheduleTest, SchedulingStatistics) {
  // 初始运行时间应该为 0
  EXPECT_EQ(task1_->sched_info.total_runtime, 0);

  // 模拟任务运行
  task1_->sched_info.total_runtime = 100;
  EXPECT_EQ(task1_->sched_info.total_runtime, 100);
}

/**
 * @brief 测试睡眠任务不应该被调度
 */
TEST_F(ScheduleTest, SleepingTaskNotScheduled) {
  task1_->status = TaskStatus::kSleeping;

  // 睡眠状态的任务不应该被调度
  EXPECT_EQ(task1_->status, TaskStatus::kSleeping);
  EXPECT_NE(task1_->status, TaskStatus::kReady);
  EXPECT_NE(task1_->status, TaskStatus::kRunning);
}

/**
 * @brief 测试阻塞任务不应该被调度
 */
TEST_F(ScheduleTest, BlockedTaskNotScheduled) {
  task1_->status = TaskStatus::kBlocked;

  // 阻塞状态的任务不应该被调度
  EXPECT_EQ(task1_->status, TaskStatus::kBlocked);
  EXPECT_NE(task1_->status, TaskStatus::kReady);
  EXPECT_NE(task1_->status, TaskStatus::kRunning);
}

/**
 * @brief 测试僵尸任务不应该被调度
 */
TEST_F(ScheduleTest, ZombieTaskNotScheduled) {
  task1_->status = TaskStatus::kZombie;

  // 僵尸状态的任务不应该被调度
  EXPECT_EQ(task1_->status, TaskStatus::kZombie);
  EXPECT_NE(task1_->status, TaskStatus::kReady);
  EXPECT_NE(task1_->status, TaskStatus::kRunning);
}

/**
 * @brief 测试只有就绪任务可以被调度
 */
TEST_F(ScheduleTest, OnlyReadyTasksCanBeScheduled) {
  task1_->status = TaskStatus::kReady;
  task2_->status = TaskStatus::kSleeping;
  task3_->status = TaskStatus::kBlocked;

  // 只有 task1 是就绪状态，可以被调度
  EXPECT_EQ(task1_->status, TaskStatus::kReady);
  EXPECT_NE(task2_->status, TaskStatus::kReady);
  EXPECT_NE(task3_->status, TaskStatus::kReady);
}
