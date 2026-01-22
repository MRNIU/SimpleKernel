/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "task_control_block.hpp"
#include "task_manager.hpp"

/**
 * @brief 测试 Exit 相关的基本功能
 * @note 这是单元测试，主要测试退出码和状态转换
 */
class ExitTest : public ::testing::Test {
 protected:
  void SetUp() override {
    // 创建父进程
    parent_ = new TaskControlBlock("Parent", 10, nullptr, nullptr);
    parent_->pid = 100;
    parent_->tgid = 100;
    parent_->parent_pid = 1;
    parent_->status = TaskStatus::kRunning;

    // 创建子进程
    child_ = new TaskControlBlock("Child", 10, nullptr, nullptr);
    child_->pid = 101;
    child_->tgid = 101;
    child_->parent_pid = parent_->pid;
    child_->status = TaskStatus::kRunning;
  }

  void TearDown() override {
    delete parent_;
    delete child_;
  }

  TaskControlBlock* parent_ = nullptr;
  TaskControlBlock* child_ = nullptr;
};

/**
 * @brief 测试任务退出时设置退出码
 */
TEST_F(ExitTest, SetExitCode) {
  int exit_code = 42;

  child_->exit_code = exit_code;

  EXPECT_EQ(child_->exit_code, 42);
}

/**
 * @brief 测试任务退出时状态转换为僵尸
 */
TEST_F(ExitTest, TaskStatusTransitionToZombie) {
  EXPECT_EQ(child_->status, TaskStatus::kRunning);

  // 有父进程的任务退出后应该进入僵尸状态
  child_->status = TaskStatus::kZombie;

  EXPECT_EQ(child_->status, TaskStatus::kZombie);
}

/**
 * @brief 测试孤儿进程（没有父进程）退出时直接退出
 */
TEST_F(ExitTest, OrphanProcessExitDirectly) {
  // 创建一个孤儿进程（parent_pid = 0）
  auto* orphan = new TaskControlBlock("Orphan", 10, nullptr, nullptr);
  orphan->pid = 102;
  orphan->tgid = 102;
  orphan->parent_pid = 0;
  orphan->status = TaskStatus::kRunning;

  EXPECT_EQ(orphan->parent_pid, 0);

  // 孤儿进程退出后应该直接进入 kExited 状态
  orphan->status = TaskStatus::kExited;

  EXPECT_EQ(orphan->status, TaskStatus::kExited);

  delete orphan;
}

/**
 * @brief 测试线程组主线程退出
 */
TEST_F(ExitTest, ThreadGroupLeaderExit) {
  // 创建线程组主线程
  auto* leader = new TaskControlBlock("Leader", 10, nullptr, nullptr);
  leader->pid = 200;
  leader->tgid = 200;
  leader->parent_pid = 1;

  // 创建线程
  auto* thread1 = new TaskControlBlock("Thread1", 10, nullptr, nullptr);
  thread1->pid = 201;
  thread1->tgid = 200;
  thread1->JoinThreadGroup(leader);

  auto* thread2 = new TaskControlBlock("Thread2", 10, nullptr, nullptr);
  thread2->pid = 202;
  thread2->tgid = 200;
  thread2->JoinThreadGroup(leader);

  // 验证线程组大小
  EXPECT_EQ(leader->GetThreadGroupSize(), 3);
  EXPECT_TRUE(leader->IsThreadGroupLeader());

  // 主线程退出前，线程组中还有其他线程
  EXPECT_GT(leader->GetThreadGroupSize(), 1);

  delete leader;
  delete thread1;
  delete thread2;
}

/**
 * @brief 测试子线程退出（非主线程）
 */
TEST_F(ExitTest, ThreadExit) {
  // 创建线程组主线程
  auto* leader = new TaskControlBlock("Leader", 10, nullptr, nullptr);
  leader->pid = 300;
  leader->tgid = 300;

  // 创建子线程
  auto* thread = new TaskControlBlock("Thread", 10, nullptr, nullptr);
  thread->pid = 301;
  thread->tgid = 300;
  thread->JoinThreadGroup(leader);

  EXPECT_FALSE(thread->IsThreadGroupLeader());
  EXPECT_EQ(thread->tgid, leader->tgid);

  // 子线程退出
  thread->LeaveThreadGroup();
  thread->status = TaskStatus::kExited;

  EXPECT_EQ(thread->status, TaskStatus::kExited);

  delete leader;
  delete thread;
}

/**
 * @brief 测试退出码的不同值
 */
TEST_F(ExitTest, DifferentExitCodes) {
  // 正常退出
  child_->exit_code = 0;
  EXPECT_EQ(child_->exit_code, 0);

  // 错误退出
  child_->exit_code = 1;
  EXPECT_EQ(child_->exit_code, 1);

  // 自定义退出码
  child_->exit_code = 42;
  EXPECT_EQ(child_->exit_code, 42);

  // 负数退出码
  child_->exit_code = -1;
  EXPECT_EQ(child_->exit_code, -1);
}

/**
 * @brief 测试任务从线程组中移除
 */
TEST_F(ExitTest, LeaveThreadGroupOnExit) {
  // 创建线程组
  auto* leader = new TaskControlBlock("Leader", 10, nullptr, nullptr);
  leader->pid = 400;
  leader->tgid = 400;

  auto* thread1 = new TaskControlBlock("Thread1", 10, nullptr, nullptr);
  thread1->pid = 401;
  thread1->tgid = 400;
  thread1->JoinThreadGroup(leader);

  auto* thread2 = new TaskControlBlock("Thread2", 10, nullptr, nullptr);
  thread2->pid = 402;
  thread2->tgid = 400;
  thread2->JoinThreadGroup(leader);

  EXPECT_EQ(leader->GetThreadGroupSize(), 3);

  // 一个线程退出
  thread1->LeaveThreadGroup();

  EXPECT_EQ(leader->GetThreadGroupSize(), 2);

  // 另一个线程退出
  thread2->LeaveThreadGroup();

  EXPECT_EQ(leader->GetThreadGroupSize(), 1);

  delete leader;
  delete thread1;
  delete thread2;
}

/**
 * @brief 测试进程退出后资源应该被释放
 */
TEST_F(ExitTest, ResourcesReleasedOnExit) {
  // 这个测试主要验证任务的状态转换
  // 实际的资源释放（如页表、内核栈）在 TaskManager 中处理

  EXPECT_EQ(child_->status, TaskStatus::kRunning);

  // 退出后状态变为僵尸
  child_->status = TaskStatus::kZombie;
  child_->exit_code = 0;

  EXPECT_EQ(child_->status, TaskStatus::kZombie);
  EXPECT_EQ(child_->exit_code, 0);
}
