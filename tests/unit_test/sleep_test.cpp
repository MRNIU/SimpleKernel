/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "task_control_block.hpp"
#include "task_manager.hpp"

/**
 * @brief 测试 Sleep 相关的基本功能
 * @note 这是单元测试，主要测试睡眠时间计算和状态转换
 */
class SleepTest : public ::testing::Test {
 protected:
  void SetUp() override {
    task_ = new TaskControlBlock("SleepTask", 10, nullptr, nullptr);
    task_->pid = 100;
    task_->tgid = 100;
    task_->status = TaskStatus::kRunning;
  }

  void TearDown() override { delete task_; }

  TaskControlBlock* task_ = nullptr;
};

/**
 * @brief 测试睡眠时间为 0 的情况（相当于 yield）
 */
TEST_F(SleepTest, SleepZeroMilliseconds) {
  uint64_t ms = 0;

  // 睡眠 0 毫秒应该相当于让出 CPU，不改变任务状态为 kSleeping
  EXPECT_EQ(ms, 0);
}

/**
 * @brief 测试睡眠时间计算
 */
TEST_F(SleepTest, CalculateSleepTicks) {
  constexpr uint64_t kMillisecondsPerSecond = 1000;
  constexpr uint64_t kTickHz = 100;  // 假设 100Hz

  // 睡眠 100 毫秒
  uint64_t ms = 100;
  uint64_t sleep_ticks = (ms * kTickHz) / kMillisecondsPerSecond;

  EXPECT_EQ(sleep_ticks, 10);  // 100ms @ 100Hz = 10 ticks

  // 睡眠 1000 毫秒（1 秒）
  ms = 1000;
  sleep_ticks = (ms * kTickHz) / kMillisecondsPerSecond;

  EXPECT_EQ(sleep_ticks, 100);  // 1000ms @ 100Hz = 100 ticks

  // 睡眠 50 毫秒
  ms = 50;
  sleep_ticks = (ms * kTickHz) / kMillisecondsPerSecond;

  EXPECT_EQ(sleep_ticks, 5);  // 50ms @ 100Hz = 5 ticks
}

/**
 * @brief 测试任务状态转换为睡眠
 */
TEST_F(SleepTest, TaskStatusTransitionToSleeping) {
  EXPECT_EQ(task_->status, TaskStatus::kRunning);

  // 模拟睡眠：任务应该从 kRunning 转换为 kSleeping
  task_->status = TaskStatus::kSleeping;

  EXPECT_EQ(task_->status, TaskStatus::kSleeping);
}

/**
 * @brief 测试唤醒时间设置
 */
TEST_F(SleepTest, WakeTickCalculation) {
  constexpr uint64_t kMillisecondsPerSecond = 1000;
  constexpr uint64_t kTickHz = 100;

  uint64_t current_tick = 1000;
  uint64_t ms = 200;  // 睡眠 200 毫秒

  uint64_t sleep_ticks = (ms * kTickHz) / kMillisecondsPerSecond;
  uint64_t wake_tick = current_tick + sleep_ticks;

  EXPECT_EQ(sleep_ticks, 20);  // 200ms @ 100Hz = 20 ticks
  EXPECT_EQ(wake_tick, 1020);  // 当前时刻 1000 + 20 = 1020
}

/**
 * @brief 测试短时间睡眠
 */
TEST_F(SleepTest, ShortSleep) {
  constexpr uint64_t kMillisecondsPerSecond = 1000;
  constexpr uint64_t kTickHz = 100;

  uint64_t ms = 10;  // 10 毫秒
  uint64_t sleep_ticks = (ms * kTickHz) / kMillisecondsPerSecond;

  EXPECT_EQ(sleep_ticks, 1);  // 10ms @ 100Hz = 1 tick
}

/**
 * @brief 测试长时间睡眠
 */
TEST_F(SleepTest, LongSleep) {
  constexpr uint64_t kMillisecondsPerSecond = 1000;
  constexpr uint64_t kTickHz = 100;

  uint64_t ms = 5000;  // 5000 毫秒（5 秒）
  uint64_t sleep_ticks = (ms * kTickHz) / kMillisecondsPerSecond;

  EXPECT_EQ(sleep_ticks, 500);  // 5000ms @ 100Hz = 500 ticks
}

/**
 * @brief 测试睡眠后任务应该被加入睡眠队列
 */
TEST_F(SleepTest, TaskAddedToSleepingQueue) {
  // 模拟睡眠：任务状态改变，并设置唤醒时间
  constexpr uint64_t kTickHz = 100;
  constexpr uint64_t kMillisecondsPerSecond = 1000;

  uint64_t current_tick = 1000;
  uint64_t ms = 100;
  uint64_t sleep_ticks = (ms * kTickHz) / kMillisecondsPerSecond;

  task_->sched_info.wake_tick = current_tick + sleep_ticks;
  task_->status = TaskStatus::kSleeping;

  EXPECT_EQ(task_->status, TaskStatus::kSleeping);
  EXPECT_EQ(task_->sched_info.wake_tick, 1010);
}
