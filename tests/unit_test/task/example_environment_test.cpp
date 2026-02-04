/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

#include "cpu_io.h"
#include "task_test_harness.hpp"
#include "test_environment_state.hpp"

/**
 * @brief 测试环境层基本功能
 */
class EnvironmentLayerTest : public TaskTestHarness {
 protected:
  void SetUp() override {
    SetNumCores(2);  // 使用双核环境
    TaskTestHarness::SetUp();
  }
};

/**
 * @brief 测试核心初始化
 */
TEST_F(EnvironmentLayerTest, CoreInitialization) {
  auto& env = GetEnvironmentState();

  ASSERT_EQ(env.GetCoreCount(), 2);

  auto& core0 = env.GetCore(0);
  EXPECT_EQ(core0.core_id, 0);
  EXPECT_TRUE(core0.interrupt_enabled);
  EXPECT_EQ(core0.interrupt_nest_level, 0);
  EXPECT_EQ(core0.page_directory, 0);
  EXPECT_FALSE(core0.paging_enabled);
  EXPECT_EQ(core0.current_thread, nullptr);
  EXPECT_EQ(core0.total_switches, 0);

  auto& core1 = env.GetCore(1);
  EXPECT_EQ(core1.core_id, 1);
}

/**
 * @brief 测试中断状态控制
 */
TEST_F(EnvironmentLayerTest, InterruptControl) {
  auto& env = GetEnvironmentState();

  // 初始状态应该是中断使能
  EXPECT_TRUE(cpu_io::GetInterruptStatus());

  // 禁用中断
  cpu_io::DisableInterrupt();
  EXPECT_FALSE(cpu_io::GetInterruptStatus());

  // 验证环境层状态
  auto& core = env.GetCurrentCoreEnv();
  EXPECT_FALSE(core.interrupt_enabled);

  // 重新使能中断
  cpu_io::EnableInterrupt();
  EXPECT_TRUE(cpu_io::GetInterruptStatus());
  EXPECT_TRUE(core.interrupt_enabled);
}

/**
 * @brief 测试页表操作
 */
TEST_F(EnvironmentLayerTest, PageTableOperations) {
  auto& env = GetEnvironmentState();

  const uint64_t test_page_dir = 0x12345000;

  // 设置页表
  cpu_io::virtual_memory::SetPageDirectory(test_page_dir);

  // 验证环境层状态
  auto& core = env.GetCurrentCoreEnv();
  EXPECT_EQ(core.page_directory, test_page_dir);
  EXPECT_EQ(cpu_io::virtual_memory::GetPageDirectory(), test_page_dir);

  // 启用分页
  cpu_io::virtual_memory::EnablePage();
  EXPECT_TRUE(core.paging_enabled);
}

/**
 * @brief 测试 CoreId 获取
 */
TEST_F(EnvironmentLayerTest, GetCoreId) {
  auto& env = GetEnvironmentState();

  // 主线程应该绑定到 core 0
  size_t core_id = cpu_io::GetCurrentCoreId();
  EXPECT_EQ(core_id, 0);

  // 验证环境层映射
  auto tid = std::this_thread::get_id();
  EXPECT_EQ(env.GetCoreIdForThread(tid), 0);
}

/**
 * @brief 测试环境状态转储（不崩溃即可）
 */
TEST_F(EnvironmentLayerTest, DumpStates) {
  auto& env = GetEnvironmentState();

  // 这个测试主要验证不会崩溃
  ASSERT_NO_THROW(env.DumpAllCoreStates());
}

/**
 * @brief 测试切换历史记录
 */
TEST_F(EnvironmentLayerTest, SwitchHistoryTracking) {
  auto& env = GetEnvironmentState();

  // 初始应该没有切换历史
  auto history = env.GetAllSwitchHistory();
  EXPECT_TRUE(history.empty());

  // 清除历史（应该不崩溃）
  ASSERT_NO_THROW(env.ClearSwitchHistory());
}
