/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_TESTS_UNIT_TEST_TASK_FIXTURES_TASK_TEST_HARNESS_HPP_
#define SIMPLEKERNEL_TESTS_UNIT_TEST_TASK_FIXTURES_TASK_TEST_HARNESS_HPP_

#include <gtest/gtest.h>

#include <cstddef>

#include "test_environment_state.hpp"

/**
 * @brief Task 模块单元测试的基类 Fixture
 *
 * 负责每个测试的 SetUp 和 TearDown，确保测试隔离
 */
class TaskTestHarness : public ::testing::Test {
 protected:
  void SetUp() override;
  void TearDown() override;

  /**
   * @brief 设置测试使用的核心数量（在 SetUp 前调用）
   */
  void SetNumCores(size_t num_cores) { num_cores_ = num_cores; }

  /**
   * @brief 获取环境层状态
   */
  auto GetEnvironmentState() -> test_env::TestEnvironmentState& {
    return test_env::TestEnvironmentState::GetInstance();
  }

  size_t num_cores_ = 1;  // 默认单核
};

#endif /* SIMPLEKERNEL_TESTS_UNIT_TEST_TASK_FIXTURES_TASK_TEST_HARNESS_HPP_ */
