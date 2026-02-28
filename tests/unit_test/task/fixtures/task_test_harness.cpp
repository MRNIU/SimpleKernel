/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task_test_harness.hpp"

#include <thread>

#include "per_cpu.hpp"
#include "task_manager.hpp"

void TaskTestHarness::SetUp() {
  // 1. 初始化环境层（每个测试都有独立实例）
  env_state_.InitializeCores(num_cores_);

  // 2. 设置当前线程的环境指针（让 Mock 层可以访问）
  env_state_.SetCurrentThreadEnvironment();

  // 3. 绑定主测试线程到 core 0
  env_state_.BindThreadToCore(std::this_thread::get_id(), 0);

  // 3. 重置 PerCpu 数据
  per_cpu::PerCpuArraySingleton::create();
  auto& per_cpu_array = per_cpu::PerCpuArraySingleton::instance();
  for (size_t i = 0; i < SIMPLEKERNEL_MAX_CORE_COUNT; ++i) {
    per_cpu_array[i] = per_cpu::PerCpu(i);
  }

  // 4. 重置 TaskManager（如果有 ResetForTesting 方法）
  // 注意：这需要在 TaskManager 中实现
  // TaskManagerSingleton::instance().ResetForTesting();
}

void TaskTestHarness::TearDown() {
  per_cpu::PerCpuArraySingleton::destroy();
  // 清除当前线程的环境指针
  env_state_.ClearCurrentThreadEnvironment();

  // 其他清理
  env_state_.ClearSwitchHistory();
}
