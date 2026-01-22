/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "task_control_block.hpp"

/**
 * @brief 测试 TaskControlBlock 与 Wait 相关的基本功能
 * @note 由于 Wait 需要 TaskManager 的完整功能，这里只测试 TCB 相关的状态和属性
 */
class TaskWaitTest : public ::testing::Test {
 protected:
  void SetUp() override {
    // 创建父进程
    parent_ = new TaskControlBlock("Parent", 10, nullptr, nullptr);
    parent_->pid = 100;
    parent_->tgid = 100;
    parent_->pgid = 100;
    parent_->parent_pid = 0;  // 假设是根进程

    // 创建子进程
    child1_ = new TaskControlBlock("Child1", 10, nullptr, nullptr);
    child1_->pid = 101;
    child1_->tgid = 101;
    child1_->pgid = 100;  // 同一进程组
    child1_->parent_pid = parent_->pid;

    child2_ = new TaskControlBlock("Child2", 10, nullptr, nullptr);
    child2_->pid = 102;
    child2_->tgid = 102;
    child2_->pgid = 100;  // 同一进程组
    child2_->parent_pid = parent_->pid;
  }

  void TearDown() override {
    delete parent_;
    delete child1_;
    delete child2_;
  }

  TaskControlBlock* parent_ = nullptr;
  TaskControlBlock* child1_ = nullptr;
  TaskControlBlock* child2_ = nullptr;
};

/**
 * @brief 测试任务状态转换
 */
TEST_F(TaskWaitTest, TaskStatusTransition) {
  // 初始状态
  EXPECT_EQ(child1_->status, TaskStatus::kReady);

  // 转换为运行状态
  child1_->status = TaskStatus::kRunning;
  EXPECT_EQ(child1_->status, TaskStatus::kRunning);

  // 转换为退出状态
  child1_->status = TaskStatus::kExited;
  EXPECT_EQ(child1_->status, TaskStatus::kExited);

  // 转换为僵尸状态
  child1_->status = TaskStatus::kZombie;
  EXPECT_EQ(child1_->status, TaskStatus::kZombie);
}

/**
 * @brief 测试退出码设置
 */
TEST_F(TaskWaitTest, ExitCode) {
  child1_->exit_code = 0;
  EXPECT_EQ(child1_->exit_code, 0);

  child1_->exit_code = 42;
  EXPECT_EQ(child1_->exit_code, 42);

  child1_->exit_code = -1;
  EXPECT_EQ(child1_->exit_code, -1);
}

/**
 * @brief 测试父子进程关系
 */
TEST_F(TaskWaitTest, ParentChildRelationship) {
  // 验证父子关系
  EXPECT_EQ(child1_->parent_pid, parent_->pid);
  EXPECT_EQ(child2_->parent_pid, parent_->pid);

  // 验证进程组
  EXPECT_EQ(child1_->pgid, parent_->pgid);
  EXPECT_EQ(child2_->pgid, parent_->pgid);
}

/**
 * @brief 测试僵尸进程状态
 */
TEST_F(TaskWaitTest, ZombieState) {
  // 模拟子进程退出
  child1_->status = TaskStatus::kZombie;
  child1_->exit_code = 0;

  // 验证状态
  EXPECT_EQ(child1_->status, TaskStatus::kZombie);
  EXPECT_EQ(child1_->exit_code, 0);

  // 父进程仍在运行
  EXPECT_EQ(parent_->status, TaskStatus::kReady);
}

/**
 * @brief 测试多个子进程的状态
 */
TEST_F(TaskWaitTest, MultipleChildrenStates) {
  // child1 退出
  child1_->status = TaskStatus::kZombie;
  child1_->exit_code = 0;

  // child2 仍在运行
  child2_->status = TaskStatus::kRunning;

  // 验证各自状态
  EXPECT_EQ(child1_->status, TaskStatus::kZombie);
  EXPECT_EQ(child2_->status, TaskStatus::kRunning);
}

/**
 * @brief 测试阻塞状态和资源 ID
 */
TEST_F(TaskWaitTest, BlockedState) {
  // 设置阻塞状态
  parent_->status = TaskStatus::kBlocked;

  // 设置阻塞资源 (假设等待子进程退出)
  auto resource_id = ResourceId(ResourceType::kChildExit, parent_->pid);
  parent_->blocked_on = resource_id;

  // 验证
  EXPECT_EQ(parent_->status, TaskStatus::kBlocked);
  EXPECT_EQ(parent_->blocked_on.GetType(), ResourceType::kChildExit);
  EXPECT_EQ(parent_->blocked_on.GetData(), parent_->pid);
}

/**
 * @brief 测试进程组 ID 匹配
 */
TEST_F(TaskWaitTest, ProcessGroupMatching) {
  // 创建不同进程组的子进程
  auto* child3 = new TaskControlBlock("Child3", 10, nullptr, nullptr);
  child3->pid = 103;
  child3->tgid = 103;
  child3->pgid = 200;  // 不同的进程组
  child3->parent_pid = parent_->pid;

  // 验证进程组
  EXPECT_EQ(child1_->pgid, parent_->pgid);
  EXPECT_EQ(child2_->pgid, parent_->pgid);
  EXPECT_NE(child3->pgid, parent_->pgid);

  delete child3;
}

/**
 * @brief 测试会话 ID
 */
TEST_F(TaskWaitTest, SessionId) {
  parent_->sid = 100;
  child1_->sid = 100;
  child2_->sid = 100;

  EXPECT_EQ(child1_->sid, parent_->sid);
  EXPECT_EQ(child2_->sid, parent_->sid);
}

/**
 * @brief 测试克隆标志位对父子关系的影响
 */
TEST_F(TaskWaitTest, CloneFlags) {
  // 模拟 fork (完全复制)
  child1_->clone_flags = kCloneAll;
  EXPECT_EQ(child1_->clone_flags, kCloneAll);

  // 模拟线程 (共享地址空间)
  child2_->clone_flags = static_cast<CloneFlags>(kCloneVm | kCloneThread);
  EXPECT_TRUE(child2_->clone_flags & kCloneVm);
  EXPECT_TRUE(child2_->clone_flags & kCloneThread);
}

/**
 * @brief 测试线程组与 Wait 的交互
 */
TEST_F(TaskWaitTest, ThreadGroupWait) {
  // 创建线程而非进程
  auto* thread1 = new TaskControlBlock("Thread1", 10, nullptr, nullptr);
  thread1->pid = 200;
  thread1->tgid = parent_->tgid;  // 同一线程组
  thread1->parent_pid = parent_->pid;
  thread1->JoinThreadGroup(parent_);

  // 验证线程组关系
  EXPECT_TRUE(parent_->InSameThreadGroup(thread1));
  EXPECT_EQ(thread1->tgid, parent_->tgid);

  // 线程退出
  thread1->status = TaskStatus::kExited;
  thread1->exit_code = 0;

  delete thread1;
}

/**
 * @brief 测试孤儿进程场景
 */
TEST_F(TaskWaitTest, OrphanProcess) {
  // 父进程退出
  parent_->status = TaskStatus::kExited;

  // 子进程变成孤儿（实际应该被 init 收养，这里只测试状态）
  EXPECT_EQ(child1_->parent_pid, parent_->pid);

  // 在实际系统中，child1->parent_pid 应该被更新为 init 的 PID
  // 但在单元测试中我们不模拟这个过程
}
