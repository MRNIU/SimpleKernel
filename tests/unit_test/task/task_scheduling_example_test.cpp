/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "arch.h"
#include "cpu_io.h"
#include "per_cpu.hpp"
#include "task_control_block.hpp"
#include "task_test_harness.hpp"
#include "test_environment_state.hpp"

/**
 * @brief 演示如何使用环境层编写任务调度测试
 */
class TaskSchedulingExample : public TaskTestHarness {
 protected:
  void SetUp() override {
    SetNumCores(1);  // 单核测试
    TaskTestHarness::SetUp();
  }

  // 简单的任务函数
  static void SimpleTaskFunc(void* arg) {
    int* counter = static_cast<int*>(arg);
    (*counter)++;
  }
};

/**
 * @brief 示例：验证任务上下文注册
 */
TEST_F(TaskSchedulingExample, TaskContextRegistration) {
  auto& env = GetEnvironmentState();

  // 创建一个任务控制块
  TaskControlBlock task;
  task.name = "TestTask";
  task.pid = 1;

  // 注册任务上下文
  env.RegisterTaskContext(&task.task_context, &task);

  // 验证可以通过上下文找到任务
  auto* found_task = env.FindTaskByContext(&task.task_context);
  ASSERT_NE(found_task, nullptr);
  EXPECT_EQ(found_task, &task);
  EXPECT_EQ(found_task->pid, 1);

  // 注销任务上下文
  env.UnregisterTaskContext(&task.task_context);

  // 验证注销后找不到任务
  found_task = env.FindTaskByContext(&task.task_context);
  EXPECT_EQ(found_task, nullptr);
}

/**
 * @brief 示例：验证 switch_to 记录切换历史
 */
TEST_F(TaskSchedulingExample, SwitchToRecordsHistory) {
  auto& env = GetEnvironmentState();

  // 创建两个任务
  TaskControlBlock task1, task2;
  task1.name = "Task1";
  task1.pid = 1;
  task2.name = "Task2";
  task2.pid = 2;

  // 注册任务上下文
  env.RegisterTaskContext(&task1.task_context, &task1);
  env.RegisterTaskContext(&task2.task_context, &task2);

  // 获取当前核心环境
  auto& core_env = env.GetCurrentCoreEnv();

  // 设置初始 tick（通过 PerCpu 的 sched_data）
  // 注意：这里假设测试已经设置了 sched_data
  // 实际使用时，应该先通过 TaskManager 初始化

  // 清空历史记录
  env.ClearSwitchHistory();

  // 执行 switch_to（从 task1 切换到 task2）
  switch_to(&task1.task_context, &task2.task_context);

  // 验证切换历史被记录
  auto history = env.GetAllSwitchHistory();
  ASSERT_EQ(history.size(), 1);

  auto& event = history[0];
  EXPECT_EQ(event.from, &task1);
  EXPECT_EQ(event.to, &task2);
  EXPECT_EQ(event.core_id, 0);

  // 验证当前线程被更新（通过 PerCpu）
  auto& per_cpu = per_cpu::GetCurrentCore();
  EXPECT_EQ(per_cpu.running_task, &task2);

  // 清理
  env.UnregisterTaskContext(&task1.task_context);
  env.UnregisterTaskContext(&task2.task_context);
}

/**
 * @brief 示例：验证多次切换的历史记录
 */
TEST_F(TaskSchedulingExample, MultipleSwitchesHistory) {
  auto& env = GetEnvironmentState();

  // 创建三个任务
  TaskControlBlock task1, task2, task3;
  task1.name = "Task1";
  task1.pid = 1;
  task2.name = "Task2";
  task2.pid = 2;
  task3.name = "Task3";
  task3.pid = 3;

  env.RegisterTaskContext(&task1.task_context, &task1);
  env.RegisterTaskContext(&task2.task_context, &task2);
  env.RegisterTaskContext(&task3.task_context, &task3);

  env.ClearSwitchHistory();

  // 执行一系列切换: task1 -> task2 -> task3 -> task1
  switch_to(&task1.task_context, &task2.task_context);
  switch_to(&task2.task_context, &task3.task_context);
  switch_to(&task3.task_context, &task1.task_context);

  // 验证历史记录
  auto history = env.GetAllSwitchHistory();
  ASSERT_EQ(history.size(), 3);

  // 第一次切换
  EXPECT_EQ(history[0].from, &task1);
  EXPECT_EQ(history[0].to, &task2);

  // 第二次切换
  EXPECT_EQ(history[1].from, &task2);
  EXPECT_EQ(history[1].to, &task3);

  // 第三次切换
  EXPECT_EQ(history[2].from, &task3);
  EXPECT_EQ(history[2].to, &task1);

  // 验证当前任务（通过 PerCpu）
  auto& per_cpu = per_cpu::GetCurrentCore();
  EXPECT_EQ(per_cpu.running_task, &task1);

  // 清理
  env.UnregisterTaskContext(&task1.task_context);
  env.UnregisterTaskContext(&task2.task_context);
  env.UnregisterTaskContext(&task3.task_context);
}

/**
 * @brief 示例：验证中断状态在任务切换时的行为
 */
TEST_F(TaskSchedulingExample, InterruptStatusDuringSwitch) {
  auto& env = GetEnvironmentState();

  // 初始状态：中断使能
  EXPECT_TRUE(cpu_io::GetInterruptStatus());

  // 模拟进入临界区（禁用中断）
  cpu_io::DisableInterrupt();
  EXPECT_FALSE(cpu_io::GetInterruptStatus());

  // 验证环境层状态
  auto& core_env = env.GetCurrentCoreEnv();
  EXPECT_FALSE(core_env.interrupt_enabled);

  // 模拟离开临界区（恢复中断）
  cpu_io::EnableInterrupt();
  EXPECT_TRUE(cpu_io::GetInterruptStatus());
  EXPECT_TRUE(core_env.interrupt_enabled);
}

/**
 * @brief 示例：验证页表切换
 */
TEST_F(TaskSchedulingExample, PageTableSwitchBetweenTasks) {
  auto& env = GetEnvironmentState();

  const uint64_t kernel_pd = 0x1000;
  const uint64_t user_task1_pd = 0x2000;
  const uint64_t user_task2_pd = 0x3000;

  // 初始内核页表
  cpu_io::virtual_memory::SetPageDirectory(kernel_pd);
  EXPECT_EQ(cpu_io::virtual_memory::GetPageDirectory(), kernel_pd);

  // 切换到用户任务1
  cpu_io::virtual_memory::SetPageDirectory(user_task1_pd);
  EXPECT_EQ(cpu_io::virtual_memory::GetPageDirectory(), user_task1_pd);

  // 切换到用户任务2
  cpu_io::virtual_memory::SetPageDirectory(user_task2_pd);
  EXPECT_EQ(cpu_io::virtual_memory::GetPageDirectory(), user_task2_pd);

  // 返回内核页表
  cpu_io::virtual_memory::SetPageDirectory(kernel_pd);
  EXPECT_EQ(cpu_io::virtual_memory::GetPageDirectory(), kernel_pd);
}

/**
 * @brief 示例：清空切换历史
 */
TEST_F(TaskSchedulingExample, ClearSwitchHistory) {
  auto& env = GetEnvironmentState();

  TaskControlBlock task1, task2;
  env.RegisterTaskContext(&task1.task_context, &task1);
  env.RegisterTaskContext(&task2.task_context, &task2);

  // 执行一些切换
  switch_to(&task1.task_context, &task2.task_context);
  switch_to(&task2.task_context, &task1.task_context);

  // 验证有历史记录
  auto history = env.GetAllSwitchHistory();
  EXPECT_GT(history.size(), 0);

  // 清空历史
  env.ClearSwitchHistory();

  // 验证历史已清空
  history = env.GetAllSwitchHistory();
  EXPECT_EQ(history.size(), 0);

  // 清理
  env.UnregisterTaskContext(&task1.task_context);
  env.UnregisterTaskContext(&task2.task_context);
}
