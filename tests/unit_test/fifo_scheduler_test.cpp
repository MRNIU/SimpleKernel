/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "fifo_scheduler.hpp"

#include <gtest/gtest.h>

#include "kstd_vector"
#include "task_control_block.hpp"

// 测试 FIFO 调度器的基本入队出队功能
TEST(FifoSchedulerTest, BasicEnqueueDequeue) {
  FifoScheduler scheduler;

  // 验证调度器名称
  EXPECT_STREQ(scheduler.name, "FIFO");

  // 创建测试任务
  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  task1.status = TaskStatus::kReady;

  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  task2.status = TaskStatus::kReady;

  // 测试空队列
  EXPECT_TRUE(scheduler.IsEmpty());
  EXPECT_EQ(scheduler.GetQueueSize(), 0);
  EXPECT_EQ(scheduler.PickNext(), nullptr);

  // 加入任务
  scheduler.Enqueue(&task1);
  EXPECT_FALSE(scheduler.IsEmpty());
  EXPECT_EQ(scheduler.GetQueueSize(), 1);

  scheduler.Enqueue(&task2);
  EXPECT_EQ(scheduler.GetQueueSize(), 2);

  // 测试 FIFO 顺序（先进先出）
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.GetQueueSize(), 1);

  EXPECT_EQ(scheduler.PickNext(), &task2);
  EXPECT_EQ(scheduler.GetQueueSize(), 0);

  EXPECT_EQ(scheduler.PickNext(), nullptr);
  EXPECT_TRUE(scheduler.IsEmpty());
}

// 测试 FIFO 的顺序性
TEST(FifoSchedulerTest, FifoOrdering) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);
  TaskControlBlock task4("Task4", 4, nullptr, nullptr);

  // 按顺序加入任务
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);
  scheduler.Enqueue(&task4);

  EXPECT_EQ(scheduler.GetQueueSize(), 4);

  // 验证严格的 FIFO 顺序
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task2);
  EXPECT_EQ(scheduler.PickNext(), &task3);
  EXPECT_EQ(scheduler.PickNext(), &task4);
  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试 Dequeue 功能（移除指定任务）
TEST(FifoSchedulerTest, DequeueSpecificTask) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);

  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);

  EXPECT_EQ(scheduler.GetQueueSize(), 3);

  // 移除中间的任务
  scheduler.Dequeue(&task2);
  EXPECT_EQ(scheduler.GetQueueSize(), 2);

  // 验证只剩下 task1 和 task3
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task3);
  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试移除队首任务
TEST(FifoSchedulerTest, DequeueFirstTask) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);

  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);

  // 移除队首任务
  scheduler.Dequeue(&task1);
  EXPECT_EQ(scheduler.GetQueueSize(), 2);

  // task2 现在应该是队首
  EXPECT_EQ(scheduler.PickNext(), &task2);
  EXPECT_EQ(scheduler.PickNext(), &task3);
  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试移除队尾任务
TEST(FifoSchedulerTest, DequeueLastTask) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);

  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);

  // 移除队尾任务
  scheduler.Dequeue(&task3);
  EXPECT_EQ(scheduler.GetQueueSize(), 2);

  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task2);
  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试统计功能
TEST(FifoSchedulerTest, Statistics) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);

  // 初始状态
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
  EXPECT_EQ(stats.total_dequeues, 0);
  EXPECT_EQ(stats.total_picks, 0);

  // 测试选择统计
  scheduler.PickNext();
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_picks, 1);

  // 测试出队统计
  scheduler.Dequeue(&task2);
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_dequeues, 1);

  // 测试抢占统计
  scheduler.OnPreempted(&task1);
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_preemptions, 1);

  // 测试重置统计
  scheduler.ResetStats();
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_enqueues, 0);
  EXPECT_EQ(stats.total_dequeues, 0);
  EXPECT_EQ(stats.total_picks, 0);
  EXPECT_EQ(stats.total_preemptions, 0);
}

// 测试重复入队同一任务
TEST(FifoSchedulerTest, RepeatedEnqueue) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);

  // 多次入队同一任务（模拟时间片用完后重新入队）
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task1);

  EXPECT_EQ(scheduler.GetQueueSize(), 3);

  // 应该能三次取出同一任务
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), &task1);
  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试混合操作
TEST(FifoSchedulerTest, MixedOperations) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  TaskControlBlock task2("Task2", 2, nullptr, nullptr);
  TaskControlBlock task3("Task3", 3, nullptr, nullptr);
  TaskControlBlock task4("Task4", 4, nullptr, nullptr);

  // 加入 task1, task2, task3
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);

  // 取出 task1
  EXPECT_EQ(scheduler.PickNext(), &task1);

  // 加入 task4
  scheduler.Enqueue(&task4);

  // 移除 task3
  scheduler.Dequeue(&task3);

  // 现在队列应该是 [task2, task4]
  EXPECT_EQ(scheduler.GetQueueSize(), 2);
  EXPECT_EQ(scheduler.PickNext(), &task2);
  EXPECT_EQ(scheduler.PickNext(), &task4);
  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试空队列操作的健壮性
TEST(FifoSchedulerTest, EmptyQueueRobustness) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);

  // 空队列操作
  EXPECT_EQ(scheduler.PickNext(), nullptr);
  EXPECT_EQ(scheduler.PickNext(), nullptr);
  EXPECT_EQ(scheduler.GetQueueSize(), 0);
  EXPECT_TRUE(scheduler.IsEmpty());

  // 尝试移除不存在的任务（应该不崩溃）
  scheduler.Dequeue(&task1);
  EXPECT_EQ(scheduler.GetQueueSize(), 0);

  // 加入后再测试
  scheduler.Enqueue(&task1);
  EXPECT_EQ(scheduler.GetQueueSize(), 1);

  // 移除存在的任务
  scheduler.Dequeue(&task1);
  EXPECT_EQ(scheduler.GetQueueSize(), 0);
  EXPECT_TRUE(scheduler.IsEmpty());
}

// 测试大量任务
TEST(FifoSchedulerTest, LargeNumberOfTasks) {
  FifoScheduler scheduler;
  constexpr size_t kTaskCount = 100;

  // 创建任务数组（使用动态分配）
  kstd::vector<TaskControlBlock*> tasks;
  for (size_t i = 0; i < kTaskCount; ++i) {
    auto* task = new TaskControlBlock("Task", 10, nullptr, nullptr);
    task->status = TaskStatus::kReady;
    tasks.push_back(task);
    scheduler.Enqueue(task);
  }

  EXPECT_EQ(scheduler.GetQueueSize(), kTaskCount);

  // 验证 FIFO 顺序
  for (size_t i = 0; i < kTaskCount; ++i) {
    auto* picked = scheduler.PickNext();
    EXPECT_NE(picked, nullptr);
    EXPECT_EQ(picked, tasks[i]);
  }

  EXPECT_EQ(scheduler.PickNext(), nullptr);
  EXPECT_TRUE(scheduler.IsEmpty());

  // 清理内存
  for (auto* task : tasks) {
    delete task;
  }
}

// 测试 OnTick 钩子（FIFO 不需要 tick 处理，应返回 false）
TEST(FifoSchedulerTest, OnTickHook) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);

  // FIFO 调度器的 OnTick 应该始终返回 false（不需要抢占）
  EXPECT_FALSE(scheduler.OnTick(&task1));
  EXPECT_FALSE(scheduler.OnTick(nullptr));
}

// 测试 OnTimeSliceExpired 钩子
TEST(FifoSchedulerTest, OnTimeSliceExpiredHook) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);

  // FIFO 调度器的 OnTimeSliceExpired 应返回 true（需要重新入队）
  EXPECT_TRUE(scheduler.OnTimeSliceExpired(&task1));
}

// 测试优先级相关钩子（FIFO 不使用优先级，但接口应该可调用）
TEST(FifoSchedulerTest, PriorityHooks) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);
  task1.sched_info.priority = 5;

  // 这些调用不应该崩溃（即使 FIFO 不使用优先级）
  scheduler.BoostPriority(&task1, 10);
  scheduler.RestorePriority(&task1);

  // 验证调度器仍正常工作
  scheduler.Enqueue(&task1);
  EXPECT_EQ(scheduler.PickNext(), &task1);
}

// 测试调度器钩子的统计
TEST(FifoSchedulerTest, SchedulerHooks) {
  FifoScheduler scheduler;

  TaskControlBlock task1("Task1", 1, nullptr, nullptr);

  // OnScheduled 和 OnPreempted 不影响队列
  scheduler.Enqueue(&task1);
  scheduler.OnScheduled(&task1);

  EXPECT_EQ(scheduler.GetQueueSize(), 1);

  auto* picked = scheduler.PickNext();
  EXPECT_EQ(picked, &task1);

  scheduler.OnPreempted(&task1);
  auto stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_preemptions, 1);
}
