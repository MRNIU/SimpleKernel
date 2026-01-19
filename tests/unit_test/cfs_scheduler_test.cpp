/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "cfs_scheduler.hpp"

#include <gtest/gtest.h>

#include "task_control_block.hpp"

// 测试 CFS 调度器的基本入队出队功能
TEST(CfsSchedulerTest, BasicEnqueueDequeue) {
  CfsScheduler scheduler;

  // 创建测试任务
  TaskControlBlock task1;
  task1.name = "Task1";
  task1.pid = 1;
  task1.status = TaskStatus::kReady;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task1.sched_data.cfs.vruntime = 0;

  TaskControlBlock task2;
  task2.name = "Task2";
  task2.pid = 2;
  task2.status = TaskStatus::kReady;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task2.sched_data.cfs.vruntime = 0;

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

  // 选择任务（vruntime 相同时，按入队顺序）
  auto* next1 = scheduler.PickNext();
  EXPECT_NE(next1, nullptr);
  EXPECT_EQ(scheduler.GetQueueSize(), 1);

  auto* next2 = scheduler.PickNext();
  EXPECT_NE(next2, nullptr);
  EXPECT_EQ(scheduler.GetQueueSize(), 0);

  EXPECT_EQ(scheduler.PickNext(), nullptr);
  EXPECT_TRUE(scheduler.IsEmpty());
}

// 测试 vruntime 排序
TEST(CfsSchedulerTest, VruntimeOrdering) {
  CfsScheduler scheduler;

  TaskControlBlock task1, task2, task3;
  task1.name = "Task1";
  task1.pid = 1;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task1.sched_data.cfs.vruntime = 1000;

  task2.name = "Task2";
  task2.pid = 2;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task2.sched_data.cfs.vruntime = 500;  // 最小 vruntime

  task3.name = "Task3";
  task3.pid = 3;
  task3.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task3.sched_data.cfs.vruntime = 750;

  // 按任意顺序加入任务
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);

  EXPECT_EQ(scheduler.GetQueueSize(), 3);

  // 应该按 vruntime 从小到大选择
  EXPECT_EQ(scheduler.PickNext(), &task2);  // vruntime = 500
  EXPECT_EQ(scheduler.PickNext(), &task3);  // vruntime = 750
  EXPECT_EQ(scheduler.PickNext(), &task1);  // vruntime = 1000
  EXPECT_EQ(scheduler.PickNext(), nullptr);
}

// 测试新任务的 vruntime 初始化
TEST(CfsSchedulerTest, NewTaskVruntimeInitialization) {
  CfsScheduler scheduler;

  TaskControlBlock task1, task2;
  task1.name = "Task1";
  task1.pid = 1;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task1.sched_data.cfs.vruntime = 1000;

  // 加入第一个任务
  scheduler.Enqueue(&task1);
  auto* picked1 = scheduler.PickNext();
  EXPECT_EQ(picked1, &task1);

  // 第二个任务 vruntime = 0 (新任务)
  task2.name = "Task2";
  task2.pid = 2;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task2.sched_data.cfs.vruntime = 0;

  scheduler.Enqueue(&task2);
  // 新任务的 vruntime 应该被设置为 min_vruntime (1000)
  EXPECT_EQ(task2.sched_data.cfs.vruntime, 1000);
}

// 测试权重对 vruntime 的影响
TEST(CfsSchedulerTest, WeightImpactOnVruntime) {
  CfsScheduler scheduler;

  TaskControlBlock task1, task2;
  task1.name = "HighPriorityTask";
  task1.pid = 1;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight * 2;  // 高优先级
  task1.sched_data.cfs.vruntime = 0;

  task2.name = "LowPriorityTask";
  task2.pid = 2;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight / 2;  // 低优先级
  task2.sched_data.cfs.vruntime = 0;

  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);

  auto* first = scheduler.PickNext();
  EXPECT_NE(first, nullptr);

  // 模拟 OnTick 更新 vruntime
  uint64_t initial_vruntime = first->sched_data.cfs.vruntime;
  scheduler.OnTick(first);

  // 权重大的任务 vruntime 增长慢
  if (first == &task1) {
    // task1 权重是 2048，delta = 1024 * 1000 / 2048 = 500
    EXPECT_EQ(first->sched_data.cfs.vruntime, initial_vruntime + 500);
  } else {
    // task2 权重是 512，delta = 1024 * 1000 / 512 = 2000
    EXPECT_EQ(first->sched_data.cfs.vruntime, initial_vruntime + 2000);
  }
}

// 测试 OnTick 抢占逻辑
TEST(CfsSchedulerTest, OnTickPreemption) {
  CfsScheduler scheduler;

  TaskControlBlock task1, task2;
  task1.name = "Task1";
  task1.pid = 1;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task1.sched_data.cfs.vruntime = 100;

  task2.name = "Task2";
  task2.pid = 2;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task2.sched_data.cfs.vruntime = 0;  // 更小的 vruntime

  scheduler.Enqueue(&task2);

  // task1 运行，vruntime 不断增长
  task1.sched_data.cfs.vruntime = 100;
  bool should_preempt = scheduler.OnTick(&task1);

  // task2 的 vruntime (0) 比 task1 小很多，应该抢占
  EXPECT_TRUE(should_preempt);
}

// 测试 OnTick 不抢占情况
TEST(CfsSchedulerTest, OnTickNoPreemption) {
  CfsScheduler scheduler;

  TaskControlBlock task1, task2;
  task1.name = "Task1";
  task1.pid = 1;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task1.sched_data.cfs.vruntime = 1000;

  task2.name = "Task2";
  task2.pid = 2;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  // OnTick 会让 task1 增加 1000，所以 task2 应该设置为 1000 + 1000 - 5 = 1995
  // 这样 OnTick 后：task1 = 2000, task2 = 1995，差距 5 < 10
  task2.sched_data.cfs.vruntime = 1995;

  scheduler.Enqueue(&task2);

  bool should_preempt = scheduler.OnTick(&task1);

  // OnTick 后差距为 5，小于 kMinGranularity (10)，不应该抢占
  EXPECT_FALSE(should_preempt);
}

// 测试 Dequeue 功能
TEST(CfsSchedulerTest, DequeueSpecificTask) {
  CfsScheduler scheduler;

  TaskControlBlock task1, task2, task3;
  task1.name = "Task1";
  task1.pid = 1;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task1.sched_data.cfs.vruntime = 100;

  task2.name = "Task2";
  task2.pid = 2;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task2.sched_data.cfs.vruntime = 200;

  task3.name = "Task3";
  task3.pid = 3;
  task3.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task3.sched_data.cfs.vruntime = 300;

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

// 测试空指针处理
TEST(CfsSchedulerTest, NullPointerHandling) {
  CfsScheduler scheduler;

  // Enqueue 空指针应该不崩溃
  scheduler.Enqueue(nullptr);
  EXPECT_EQ(scheduler.PickNext(), nullptr);

  // Dequeue 空指针应该不崩溃
  scheduler.Dequeue(nullptr);

  // OnTick 空指针应该不崩溃
  EXPECT_FALSE(scheduler.OnTick(nullptr));

  // OnPreempted 空指针应该不崩溃
  scheduler.OnPreempted(nullptr);

  // OnScheduled 空指针应该不崩溃
  scheduler.OnScheduled(nullptr);
}

// 测试默认权重设置
TEST(CfsSchedulerTest, DefaultWeightAssignment) {
  CfsScheduler scheduler;

  TaskControlBlock task;
  task.name = "Task";
  task.pid = 1;
  task.sched_data.cfs.weight = 0;  // 权重未设置
  task.sched_data.cfs.vruntime = 0;

  scheduler.Enqueue(&task);

  // 权重应该被设置为默认值
  EXPECT_EQ(task.sched_data.cfs.weight, CfsScheduler::kDefaultWeight);
}

// 测试统计信息
TEST(CfsSchedulerTest, Statistics) {
  CfsScheduler scheduler;

  TaskControlBlock task1, task2;
  task1.name = "Task1";
  task1.pid = 1;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task1.sched_data.cfs.vruntime = 0;

  task2.name = "Task2";
  task2.pid = 2;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task2.sched_data.cfs.vruntime = 0;

  // 测试 enqueue 计数
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  auto stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_enqueues, 2);

  // 测试 pick 计数
  scheduler.PickNext();
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_picks, 1);

  scheduler.PickNext();
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_picks, 2);

  // 测试 dequeue 计数
  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Dequeue(&task1);
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_dequeues, 1);

  // 测试重置统计
  scheduler.ResetStats();
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_enqueues, 0);
  EXPECT_EQ(stats.total_picks, 0);
  EXPECT_EQ(stats.total_dequeues, 0);
  EXPECT_EQ(stats.total_preemptions, 0);
}

// 测试 min_vruntime 更新
TEST(CfsSchedulerTest, MinVruntimeUpdate) {
  CfsScheduler scheduler;

  TaskControlBlock task1, task2, task3;
  task1.name = "Task1";
  task1.pid = 1;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task1.sched_data.cfs.vruntime = 1000;

  task2.name = "Task2";
  task2.pid = 2;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task2.sched_data.cfs.vruntime = 500;

  task3.name = "Task3";
  task3.pid = 3;
  task3.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task3.sched_data.cfs.vruntime = 750;

  scheduler.Enqueue(&task1);
  scheduler.Enqueue(&task2);
  scheduler.Enqueue(&task3);

  // 初始 min_vruntime 应该是 0
  EXPECT_EQ(scheduler.GetMinVruntime(), 0);

  // 选择 task2 (vruntime = 500)
  scheduler.PickNext();

  // min_vruntime 应该更新为队列中最小的 (750)
  EXPECT_EQ(scheduler.GetMinVruntime(), 750);

  // 选择 task3 (vruntime = 750)
  scheduler.PickNext();

  // min_vruntime 应该更新为 1000
  EXPECT_EQ(scheduler.GetMinVruntime(), 1000);

  // 选择 task1 (vruntime = 1000)
  scheduler.PickNext();

  // 队列为空，min_vruntime 保持为 1000
  EXPECT_EQ(scheduler.GetMinVruntime(), 1000);
}

// 测试多次 OnTick 的 vruntime 累积
TEST(CfsSchedulerTest, MultipleTicksVruntimeAccumulation) {
  CfsScheduler scheduler;

  TaskControlBlock task;
  task.name = "Task";
  task.pid = 1;
  task.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task.sched_data.cfs.vruntime = 0;

  // 模拟多次 tick
  constexpr int kTickCount = 10;
  uint64_t expected_delta = (CfsScheduler::kDefaultWeight * 1000) /
                            task.sched_data.cfs.weight;  // = 1000

  for (int i = 0; i < kTickCount; ++i) {
    scheduler.OnTick(&task);
  }

  EXPECT_EQ(task.sched_data.cfs.vruntime, expected_delta * kTickCount);
}

// 测试不同权重下的公平性
TEST(CfsSchedulerTest, FairnessWithDifferentWeights) {
  CfsScheduler scheduler;

  TaskControlBlock high_priority, low_priority;
  high_priority.name = "HighPriority";
  high_priority.pid = 1;
  high_priority.sched_data.cfs.weight =
      CfsScheduler::kDefaultWeight * 2;  // 2倍权重
  high_priority.sched_data.cfs.vruntime = 0;

  low_priority.name = "LowPriority";
  low_priority.pid = 2;
  low_priority.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;  // 1倍权重
  low_priority.sched_data.cfs.vruntime = 0;

  // 模拟相同次数的 tick
  constexpr int kTickCount = 10;

  for (int i = 0; i < kTickCount; ++i) {
    scheduler.OnTick(&high_priority);
    scheduler.OnTick(&low_priority);
  }

  // 高优先级任务的 vruntime 增长应该是低优先级的一半
  // high: delta = 1024 * 1000 / 2048 = 500 per tick
  // low:  delta = 1024 * 1000 / 1024 = 1000 per tick
  EXPECT_EQ(high_priority.sched_data.cfs.vruntime, 500 * kTickCount);
  EXPECT_EQ(low_priority.sched_data.cfs.vruntime, 1000 * kTickCount);
  EXPECT_EQ(low_priority.sched_data.cfs.vruntime,
            high_priority.sched_data.cfs.vruntime * 2);
}

// 测试极端权重值
TEST(CfsSchedulerTest, ExtremeWeightValues) {
  CfsScheduler scheduler;

  TaskControlBlock task;
  task.name = "Task";
  task.pid = 1;
  task.sched_data.cfs.vruntime = 0;

  // 测试极小权重 (避免除零)
  task.sched_data.cfs.weight = 1;
  scheduler.OnTick(&task);
  EXPECT_GT(task.sched_data.cfs.vruntime, 0);

  // 测试极大权重
  task.sched_data.cfs.vruntime = 0;
  task.sched_data.cfs.weight = CfsScheduler::kDefaultWeight * 1000;
  scheduler.OnTick(&task);
  EXPECT_GT(task.sched_data.cfs.vruntime, 0);
  EXPECT_LT(task.sched_data.cfs.vruntime, 10);  // 应该很小
}

// 测试队列大小的一致性
TEST(CfsSchedulerTest, QueueSizeConsistency) {
  CfsScheduler scheduler;

  TaskControlBlock tasks[5];
  for (int i = 0; i < 5; ++i) {
    tasks[i].pid = i;
    tasks[i].sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
    tasks[i].sched_data.cfs.vruntime = i * 100;
  }

  // 加入 5 个任务
  for (auto& task : tasks) {
    scheduler.Enqueue(&task);
  }
  EXPECT_EQ(scheduler.GetQueueSize(), 5);

  // 移除 3 个任务
  scheduler.PickNext();
  scheduler.PickNext();
  scheduler.PickNext();
  EXPECT_EQ(scheduler.GetQueueSize(), 2);

  // 加入 2 个任务
  scheduler.Enqueue(&tasks[0]);
  scheduler.Enqueue(&tasks[1]);
  EXPECT_EQ(scheduler.GetQueueSize(), 4);

  // 清空队列
  while (!scheduler.IsEmpty()) {
    scheduler.PickNext();
  }
  EXPECT_EQ(scheduler.GetQueueSize(), 0);
  EXPECT_TRUE(scheduler.IsEmpty());
}

// 测试抢占统计
TEST(CfsSchedulerTest, PreemptionStatistics) {
  CfsScheduler scheduler;

  TaskControlBlock task1, task2;
  task1.name = "Task1";
  task1.pid = 1;
  task1.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task1.sched_data.cfs.vruntime = 1000;

  task2.name = "Task2";
  task2.pid = 2;
  task2.sched_data.cfs.weight = CfsScheduler::kDefaultWeight;
  task2.sched_data.cfs.vruntime = 0;

  scheduler.Enqueue(&task2);

  // 触发抢占
  scheduler.OnTick(&task1);
  auto stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_preemptions, 1);

  // OnPreempted 也会增加抢占计数
  scheduler.OnPreempted(&task1);
  stats = scheduler.GetStats();
  EXPECT_EQ(stats.total_preemptions, 2);
}
