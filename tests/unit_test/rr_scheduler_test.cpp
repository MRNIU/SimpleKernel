/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "rr_scheduler.hpp"

#include <gtest/gtest.h>

#include "sk_vector"
#include "task_control_block.hpp"

// 测试 Round-Robin 调度器的基本功能
TEST(RoundRobinSchedulerTest, BasicEnqueueDequeue) {
  RoundRobinScheduler scheduler;

  // 创建测试任务
  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  task1.status = TaskStatus::kReady;

  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  task2.status = TaskStatus::kReady;

  // 测试空队列
  EXPECT_EQ(scheduler.PickNext(), nullptr);

  // 加入任务
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);

  // 测试 FIFO 顺序
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task2);
  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试 Round-Robin 的轮转特性
TEST(RoundRobinSchedulerTest, RoundRobinRotation) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);

  // 加入三个任务
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);

  // 第一轮
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task2);
  EXPECT_EQ(scheduler.PickNext(), &task3);

  // 模拟时间片用完，任务重新入队
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);

  // 第二轮 - 应该保持相同的顺序
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task2);
  EXPECT_EQ(scheduler.PickNext(), &task3);
}

// 测试 Dequeue 功能
TEST(RoundRobinSchedulerTest, DequeueSpecificTask) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);

  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);

  // 移除中间的任务
  scheduler.Dequeue(&task2);

  // 验证只剩下 task1 和 task3
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task3);
  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试空指针处理
TEST(RoundRobinSchedulerTest, NullPointerHandling) {
  RoundRobinScheduler scheduler;

  // Enqueue 空指针应该不崩溃
  scheduler.Enqueue(nullptr);
  EXPECT_EQ(scheduler.PickNext(), nullptr);

  // Dequeue 空指针应该不崩溃
  scheduler.Dequeue(nullptr);
}

// 测试重复入队和出队
TEST(RoundRobinSchedulerTest, RepeatedEnqueueDequeue) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);

  // 重复入队和出队
  for (int i = 0; i < 10; ++i) {
    scheduler.Enqueue(&task1);
    EXPECT_EQ(scheduler.PickNext(), &task1);
  }

  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试多任务交替入队
TEST(RoundRobinSchedulerTest, InterleavedEnqueueDequeue) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);

  scheduler.Enqueue(&task1);
  EXPECT_EQ(scheduler.PickNext(), &task1);

  scheduler.Enqueue(&task2);
  EXPECT_EQ(scheduler.PickNext(), &task2);

  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task2);
}

// 测试队列大小和空状态检查
TEST(RoundRobinSchedulerTest, QueueSizeAndEmpty) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);

  // 初始状态
  EXPECT_TRUE(scheduler.IsEmpty());
  EXPECT_EQ(scheduler.GetQueueSize(), 0);

  // 加入任务
  scheduler.Enqueue(&task1);
  EXPECT_FALSE(scheduler.IsEmpty());
  EXPECT_EQ(scheduler.GetQueueSize(), 1);

  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);
  EXPECT_EQ(scheduler.GetQueueSize(), 3);

  // 取出任务
  scheduler.PickNext();
  EXPECT_EQ(scheduler.GetQueueSize(), 2);

  scheduler.PickNext();
  scheduler.PickNext();
  EXPECT_TRUE(scheduler.IsEmpty());
  EXPECT_EQ(scheduler.GetQueueSize(), 0);
}

// 测试时间片重置
TEST(RoundRobinSchedulerTest, TimeSliceReset) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  task1.sched_info.time_slice_default = 20;
  task1.sched_info.time_slice_remaining = 5;  // 时间片快用完了

  // 入队应该重置时间片
  scheduler.Enqueue(&task1);
  EXPECT_EQ(task1.sched_info.time_slice_remaining, 20);

  // 模拟时间片耗尽
  task1.sched_info.time_slice_remaining = 0;
  bool should_reenqueue = scheduler.OnTimeSliceExpired(&task1);
  EXPECT_TRUE(should_reenqueue);
  EXPECT_EQ(task1.sched_info.time_slice_remaining, 20);
}

// 测试统计信息
TEST(RoundRobinSchedulerTest, Statistics) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);

  // 初始统计
  auto stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_enqueues, 0);
  EXPECT_EQ(stats.total_dequeues, 0);
  EXPECT_EQ(stats.total_picks, 0);
  EXPECT_EQ(stats.total_preemptions, 0);

  // 测试入队统计
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_enqueues, 2);

  // 测试选择统计
  scheduler.PickNext();
  scheduler.PickNext();
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_picks, 2);

  // 测试出队统计
  scheduler.Enqueue(&task1);
  scheduler.Dequeue(&task1);
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_dequeues, 1);

  // 测试抢占统计
  scheduler.OnPreempted(&task1);
  scheduler.OnPreempted(&task2);
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_preemptions, 2);

  // 重置统计
  scheduler.ResetStats();
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_enqueues, 0);
  EXPECT_EQ(stats.total_dequeues, 0);
  EXPECT_EQ(stats.total_picks, 0);
  EXPECT_EQ(stats.total_preemptions, 0);
}

// 测试大量任务的公平性
TEST(RoundRobinSchedulerTest, FairnessWithManyTasks) {
  RoundRobinScheduler scheduler;
  constexpr size_t kTaskCount = 100;

  // 创建任务数组（使用动态分配）
  sk_std::vector<TaskControlBlock*> tasks;
  for (size_t i = 0; i < kTaskCount; ++i) {
    auto* task = new TaskControlBlock("Task", i, nullptr, nullptr);
    task->status = TaskStatus::kReady;
    tasks.push_back(task);
    scheduler.Enqueue(task);
  }

  EXPECT_EQ(scheduler.GetQueueSize(), kTaskCount);

  // 验证所有任务按顺序被选中
  for (size_t i = 0; i < kTaskCount; ++i) {
    auto* picked = scheduler.PickNext();
    ASSERT_NE(picked, nullptr);
    EXPECT_EQ(picked->pid, i);
  }

  EXPECT_TRUE(scheduler.IsEmpty());

  // 清理内存
  for (auto* task : tasks) {
    delete task;
  }
}

// 测试多轮轮转
TEST(RoundRobinSchedulerTest, MultipleRounds) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);

  // 进行 5 轮轮转
  for (int round = 0; round < 5; ++round) {
    scheduler.Enqueue(&task1);
    scheduler.Enqueue(&task2);
    scheduler.Enqueue(&task3);

    EXPECT_EQ(scheduler.PickNext()->pid, 1);
    EXPECT_EQ(scheduler.PickNext()->pid, 2);
    EXPECT_EQ(scheduler.PickNext()->pid, 3);
    EXPECT_TRUE(scheduler.IsEmpty());
  }
}

// 测试边界条件：单个任务
TEST(RoundRobinSchedulerTest, SingleTask) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);

  scheduler.Enqueue(&task1);
  EXPECT_EQ(scheduler.GetQueueSize(), 1);

  auto* picked = scheduler.PickNext();
  EXPECT_EQ(picked, &task1);
  EXPECT_TRUE(scheduler.IsEmpty());
}

// 测试出队不存在的任务
TEST(RoundRobinSchedulerTest, DequeueNonExistentTask) {
  RoundRobinScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);

  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);

  // 尝试移除不在队列中的任务
  scheduler.Dequeue(&task3);
  EXPECT_EQ(scheduler.GetQueueSize(), 2);

  // 验证原有任务仍在队列中
  EXPECT_EQ(scheduler.PickNext()->pid, 1);
  EXPECT_EQ(scheduler.PickNext()->pid, 2);
  EXPECT_TRUE(scheduler.IsEmpty());
}
