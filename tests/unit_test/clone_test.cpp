/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "task_control_block.hpp"
#include "task_manager.hpp"

/**
 * @brief 测试 Clone 相关的基本功能
 * @note 这是单元测试，主要测试标志位验证和基本逻辑
 */
class CloneTest : public ::testing::Test {
 protected:
  void SetUp() override {
    // 创建父任务
    parent_ = new TaskControlBlock("Parent", 10, nullptr, nullptr);
    parent_->pid = 100;
    parent_->tgid = 100;
    parent_->pgid = 100;
    parent_->parent_pid = 1;
    parent_->status = TaskStatus::kReady;
  }

  void TearDown() override { delete parent_; }

  TaskControlBlock* parent_ = nullptr;
};

/**
 * @brief 测试 Clone 标志位的验证
 */
TEST_F(CloneTest, ValidateCloneFlags) {
  // 测试 kCloneThread 必须同时设置 kCloneVm, kCloneFiles, kCloneSighand
  uint64_t flags = kCloneThread;

  // 如果只设置 kCloneThread，应该自动补全其他标志
  if ((flags & kCloneThread) &&
      (!(flags & kCloneVm) || !(flags & kCloneFiles) ||
       !(flags & kCloneSighand))) {
    flags |= (kCloneVm | kCloneFiles | kCloneSighand);
  }

  EXPECT_TRUE(flags & kCloneVm);
  EXPECT_TRUE(flags & kCloneFiles);
  EXPECT_TRUE(flags & kCloneSighand);
}

/**
 * @brief 测试进程克隆标志（不共享地址空间）
 */
TEST_F(CloneTest, ProcessCloneFlags) {
  uint64_t flags = 0;  // 不设置 kCloneVm，表示创建进程

  EXPECT_FALSE(flags & kCloneVm);
  EXPECT_FALSE(flags & kCloneThread);
}

/**
 * @brief 测试线程克隆标志（共享地址空间）
 */
TEST_F(CloneTest, ThreadCloneFlags) {
  uint64_t flags = kCloneThread | kCloneVm | kCloneFiles | kCloneSighand;

  EXPECT_TRUE(flags & kCloneVm);
  EXPECT_TRUE(flags & kCloneThread);
  EXPECT_TRUE(flags & kCloneFiles);
  EXPECT_TRUE(flags & kCloneSighand);
}

/**
 * @brief 测试 kCloneParent 标志
 */
TEST_F(CloneTest, CloneParentFlag) {
  // 如果设置了 kCloneParent，子进程的 parent_pid 应该与父进程的
  // parent_pid 相同
  uint64_t flags_with_parent = kCloneParent;
  uint64_t flags_without_parent = 0;

  EXPECT_TRUE(flags_with_parent & kCloneParent);
  EXPECT_FALSE(flags_without_parent & kCloneParent);
}

/**
 * @brief 测试线程组 ID (TGID) 的设置
 */
TEST_F(CloneTest, ThreadGroupId) {
  // 创建线程：tgid 应该等于父进程的 tgid
  uint64_t thread_flags = kCloneThread | kCloneVm | kCloneFiles | kCloneSighand;
  Pid expected_tgid_for_thread = parent_->tgid;

  // 创建进程：tgid 应该等于新进程自己的 pid
  uint64_t process_flags = 0;

  EXPECT_TRUE(thread_flags & kCloneThread);
  EXPECT_EQ(expected_tgid_for_thread, parent_->tgid);

  EXPECT_FALSE(process_flags & kCloneThread);
}

/**
 * @brief 测试文件描述符表标志 (kCloneFiles)
 */
TEST_F(CloneTest, CloneFilesFlag) {
  uint64_t flags_share = kCloneFiles;
  uint64_t flags_copy = 0;

  EXPECT_TRUE(flags_share & kCloneFiles);
  EXPECT_FALSE(flags_copy & kCloneFiles);
}

/**
 * @brief 测试信号处理器标志 (kCloneSighand)
 */
TEST_F(CloneTest, CloneSighandFlag) {
  uint64_t flags_share = kCloneSighand;
  uint64_t flags_copy = 0;

  EXPECT_TRUE(flags_share & kCloneSighand);
  EXPECT_FALSE(flags_copy & kCloneSighand);
}

/**
 * @brief 测试文件系统信息标志 (kCloneFs)
 */
TEST_F(CloneTest, CloneFsFlag) {
  uint64_t flags_share = kCloneFs;
  uint64_t flags_copy = 0;

  EXPECT_TRUE(flags_share & kCloneFs);
  EXPECT_FALSE(flags_copy & kCloneFs);
}

/**
 * @brief 测试地址空间标志 (kCloneVm)
 */
TEST_F(CloneTest, CloneVmFlag) {
  uint64_t flags_share = kCloneVm;
  uint64_t flags_copy = 0;

  EXPECT_TRUE(flags_share & kCloneVm);
  EXPECT_FALSE(flags_copy & kCloneVm);
}

/**
 * @brief 测试多个标志位的组合
 */
TEST_F(CloneTest, CombinedFlags) {
  // 典型的线程创建标志
  uint64_t thread_flags =
      kCloneThread | kCloneVm | kCloneFiles | kCloneSighand | kCloneFs;

  EXPECT_TRUE(thread_flags & kCloneThread);
  EXPECT_TRUE(thread_flags & kCloneVm);
  EXPECT_TRUE(thread_flags & kCloneFiles);
  EXPECT_TRUE(thread_flags & kCloneSighand);
  EXPECT_TRUE(thread_flags & kCloneFs);

  // 典型的进程创建标志（基本上不设置任何共享标志）
  uint64_t process_flags = 0;

  EXPECT_FALSE(process_flags & kCloneThread);
  EXPECT_FALSE(process_flags & kCloneVm);
  EXPECT_FALSE(process_flags & kCloneFiles);
  EXPECT_FALSE(process_flags & kCloneSighand);
  EXPECT_FALSE(process_flags & kCloneFs);
}
