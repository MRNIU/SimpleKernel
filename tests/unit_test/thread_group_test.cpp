/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "task_control_block.hpp"

/**
 * @brief 测试线程组的基本功能
 */
class ThreadGroupTest : public ::testing::Test {
 protected:
  void SetUp() override {
    // 创建主线程和子线程
    leader_ = new TaskControlBlock("Leader", 10, nullptr, nullptr);
    leader_->pid = 100;
    leader_->tgid = 100;  // 主线程的 tgid 等于自己的 pid

    thread1_ = new TaskControlBlock("Thread1", 10, nullptr, nullptr);
    thread1_->pid = 101;

    thread2_ = new TaskControlBlock("Thread2", 10, nullptr, nullptr);
    thread2_->pid = 102;

    thread3_ = new TaskControlBlock("Thread3", 10, nullptr, nullptr);
    thread3_->pid = 103;
  }

  void TearDown() override {
    delete leader_;
    delete thread1_;
    delete thread2_;
    delete thread3_;
  }

  TaskControlBlock* leader_ = nullptr;
  TaskControlBlock* thread1_ = nullptr;
  TaskControlBlock* thread2_ = nullptr;
  TaskControlBlock* thread3_ = nullptr;
};

/**
 * @brief 测试 IsThreadGroupLeader
 */
TEST_F(ThreadGroupTest, IsThreadGroupLeader) {
  EXPECT_TRUE(leader_->IsThreadGroupLeader());
  EXPECT_FALSE(thread1_->IsThreadGroupLeader());
}

/**
 * @brief 测试加入线程组
 */
TEST_F(ThreadGroupTest, JoinThreadGroup) {
  // 线程 1 加入主线程的线程组
  thread1_->JoinThreadGroup(leader_);

  // 验证 tgid
  EXPECT_EQ(thread1_->tgid, leader_->tgid);
  EXPECT_EQ(thread1_->tgid, 100);

  // 验证链表结构
  EXPECT_EQ(leader_->thread_group_next, thread1_);
  EXPECT_EQ(thread1_->thread_group_prev, leader_);
  EXPECT_EQ(thread1_->thread_group_next, nullptr);
}

/**
 * @brief 测试多个线程加入线程组
 */
TEST_F(ThreadGroupTest, MultipleThreadsJoin) {
  // 多个线程依次加入
  thread1_->JoinThreadGroup(leader_);
  thread2_->JoinThreadGroup(leader_);
  thread3_->JoinThreadGroup(leader_);

  // 验证所有线程的 tgid
  EXPECT_EQ(thread1_->tgid, 100);
  EXPECT_EQ(thread2_->tgid, 100);
  EXPECT_EQ(thread3_->tgid, 100);

  // 验证链表结构 (后加入的插在前面)
  EXPECT_EQ(leader_->thread_group_next, thread3_);
  EXPECT_EQ(thread3_->thread_group_prev, leader_);
  EXPECT_EQ(thread3_->thread_group_next, thread2_);
  EXPECT_EQ(thread2_->thread_group_prev, thread3_);
  EXPECT_EQ(thread2_->thread_group_next, thread1_);
  EXPECT_EQ(thread1_->thread_group_prev, thread2_);
  EXPECT_EQ(thread1_->thread_group_next, nullptr);
}

/**
 * @brief 测试获取线程组大小
 */
TEST_F(ThreadGroupTest, GetThreadGroupSize) {
  // 未加入线程组
  EXPECT_EQ(thread1_->GetThreadGroupSize(), 1);

  // 加入后
  thread1_->JoinThreadGroup(leader_);
  EXPECT_EQ(leader_->GetThreadGroupSize(), 2);
  EXPECT_EQ(thread1_->GetThreadGroupSize(), 2);

  thread2_->JoinThreadGroup(leader_);
  EXPECT_EQ(leader_->GetThreadGroupSize(), 3);
  EXPECT_EQ(thread1_->GetThreadGroupSize(), 3);
  EXPECT_EQ(thread2_->GetThreadGroupSize(), 3);

  thread3_->JoinThreadGroup(leader_);
  EXPECT_EQ(leader_->GetThreadGroupSize(), 4);
  EXPECT_EQ(thread3_->GetThreadGroupSize(), 4);
}

/**
 * @brief 测试离开线程组
 */
TEST_F(ThreadGroupTest, LeaveThreadGroup) {
  // 先加入
  thread1_->JoinThreadGroup(leader_);
  thread2_->JoinThreadGroup(leader_);
  thread3_->JoinThreadGroup(leader_);

  // 验证初始大小
  EXPECT_EQ(leader_->GetThreadGroupSize(), 4);

  // 中间的线程离开
  thread2_->LeaveThreadGroup();
  EXPECT_EQ(thread2_->thread_group_prev, nullptr);
  EXPECT_EQ(thread2_->thread_group_next, nullptr);
  EXPECT_EQ(leader_->GetThreadGroupSize(), 3);

  // 验证链表修复
  EXPECT_EQ(thread3_->thread_group_next, thread1_);
  EXPECT_EQ(thread1_->thread_group_prev, thread3_);

  // 第一个线程离开
  thread3_->LeaveThreadGroup();
  EXPECT_EQ(leader_->thread_group_next, thread1_);
  EXPECT_EQ(thread1_->thread_group_prev, leader_);
  EXPECT_EQ(leader_->GetThreadGroupSize(), 2);

  // 最后一个线程离开
  thread1_->LeaveThreadGroup();
  EXPECT_EQ(leader_->thread_group_next, nullptr);
  EXPECT_EQ(leader_->GetThreadGroupSize(), 1);
}

/**
 * @brief 测试 InSameThreadGroup
 */
TEST_F(ThreadGroupTest, InSameThreadGroup) {
  // 未加入线程组时
  EXPECT_FALSE(leader_->InSameThreadGroup(thread1_));
  EXPECT_FALSE(thread1_->InSameThreadGroup(leader_));

  // 加入线程组后
  thread1_->JoinThreadGroup(leader_);
  thread2_->JoinThreadGroup(leader_);

  EXPECT_TRUE(leader_->InSameThreadGroup(thread1_));
  EXPECT_TRUE(thread1_->InSameThreadGroup(leader_));
  EXPECT_TRUE(thread1_->InSameThreadGroup(thread2_));
  EXPECT_TRUE(thread2_->InSameThreadGroup(thread1_));

  // thread3 未加入
  EXPECT_FALSE(thread1_->InSameThreadGroup(thread3_));
  EXPECT_FALSE(thread3_->InSameThreadGroup(thread1_));

  // nullptr 测试
  EXPECT_FALSE(thread1_->InSameThreadGroup(nullptr));
}

/**
 * @brief 测试重复加入同一线程组
 */
TEST_F(ThreadGroupTest, JoinSameLeaderTwice) {
  thread1_->JoinThreadGroup(leader_);

  // 记录当前状态
  auto original_tgid = thread1_->tgid;
  auto original_prev = thread1_->thread_group_prev;

  // 再次加入同一个 leader (应该被忽略或重新插入)
  thread1_->JoinThreadGroup(leader_);

  // tgid 不应改变
  EXPECT_EQ(thread1_->tgid, original_tgid);
}

/**
 * @brief 测试边界情况：自己加入自己
 */
TEST_F(ThreadGroupTest, JoinSelf) {
  leader_->JoinThreadGroup(leader_);

  // 不应该改变任何状态
  EXPECT_EQ(leader_->thread_group_next, nullptr);
  EXPECT_EQ(leader_->thread_group_prev, nullptr);
  EXPECT_EQ(leader_->tgid, 100);
}

/**
 * @brief 测试边界情况：加入 nullptr
 */
TEST_F(ThreadGroupTest, JoinNullptr) {
  thread1_->JoinThreadGroup(nullptr);

  // 不应该改变任何状态
  EXPECT_EQ(thread1_->thread_group_next, nullptr);
  EXPECT_EQ(thread1_->thread_group_prev, nullptr);
  EXPECT_EQ(thread1_->tgid, 0);
}

/**
 * @brief 测试析构时自动离开线程组
 */
TEST_F(ThreadGroupTest, AutoLeaveOnDestroy) {
  auto* temp_thread = new TaskControlBlock("Temp", 10, nullptr, nullptr);
  temp_thread->pid = 200;
  temp_thread->JoinThreadGroup(leader_);

  EXPECT_EQ(leader_->GetThreadGroupSize(), 2);

  // 析构时应该自动离开线程组
  delete temp_thread;

  // 验证链表已修复
  EXPECT_EQ(leader_->thread_group_next, nullptr);
  EXPECT_EQ(leader_->GetThreadGroupSize(), 1);
}
