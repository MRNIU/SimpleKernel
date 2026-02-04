/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task_test_harness.hpp"

#include <thread>

#include "per_cpu.hpp"
#include "singleton.hpp"
#include "task_manager.hpp"

void TaskTestHarness::SetUp() {
  // 1. 重置环境层
  auto& env_state = test_env::TestEnvironmentState::GetInstance();
  env_state.ResetAllCores();
  env_state.InitializeCores(num_cores_);

  // 2. 绑定主测试线程到 core 0
  env_state.BindThreadToCore(std::this_thread::get_id(), 0);

  // 3. 重置 PerCpu 数据
  auto& per_cpu_array = Singleton<
      std::array<per_cpu::PerCpu, SIMPLEKERNEL_MAX_CORE_COUNT>>::GetInstance();
  for (size_t i = 0; i < SIMPLEKERNEL_MAX_CORE_COUNT; ++i) {
    per_cpu_array[i] = per_cpu::PerCpu(i);
  }

  // 4. 重置 TaskManager（如果有 ResetForTesting 方法）
  // 注意：这需要在 TaskManager 中实现
  // Singleton<TaskManager>::GetInstance().ResetForTesting();
}

void TaskTestHarness::TearDown() {
  // 清理环境
  auto& env_state = test_env::TestEnvironmentState::GetInstance();
  env_state.ClearSwitchHistory();
  env_state.ResetAllCores();
}
