/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "scheduler/rr_scheduler.hpp"

#include <gtest/gtest.h>

#include "task_control_block.hpp"

// 测试 Round-Robin 调度器的基本功能
TEST(RoundRobinSchedulerTest, BasicEnqueueDequeue) {
  RoundRobinScheduler scheduler;

  // 创建测试任务
  TaskControlBlock task1;
  task1.name = "Task1";
  task1.pid = 1;
  task1.status = TaskStatus::kReady;

  TaskControlBlock task2;
  task2.name = "Task2";
  task2.pid = 2;
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

  TaskControlBlock task1, task2, task3;
  task1.name = "Task1";
  task1.pid = 1;
  task2.name = "Task2";
  task2.pid = 2;
  task3.name = "Task3";
  task3.pid = 3;

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

  TaskControlBlock task1, task2, task3;
  task1.name = "Task1";
  task1.pid = 1;
  task2.name = "Task2";
  task2.pid = 2;
  task3.name = "Task3";
  task3.pid = 3;

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

  TaskControlBlock task1;
  task1.name = "Task1";
  task1.pid = 1;

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

  TaskControlBlock task1, task2;
  task1.name = "Task1";
  task1.pid = 1;
  task2.name = "Task2";
  task2.pid = 2;

  scheduler.Enqueue(&task1);
  EXPECT_EQ(scheduler.PickNext(), &task1);

  scheduler.Enqueue(&task2);
  EXPECT_EQ(scheduler.PickNext(), &task2);

  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task2);
}
