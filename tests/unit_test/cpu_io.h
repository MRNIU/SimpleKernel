/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_TESTS_UNIT_TEST_CPU_IO_H_
#define SIMPLEKERNEL_TESTS_UNIT_TEST_CPU_IO_H_

#include <atomic>
#include <cstddef>
#include <thread>
#include <unordered_map>

namespace cpu_io {

inline void Pause() {}

// 使用内部函数的静态变量来避免多重定义
inline auto GetInterruptStatusRef() -> bool& {
  thread_local bool interrupt_enabled = true;
  return interrupt_enabled;
}

// 使用线程 ID 映射到核心 ID（用于测试多核场景）
inline auto GetCurrentCoreId() -> size_t {
  // 使用线程 ID 的哈希值作为核心 ID
  return std::hash<std::thread::id>{}(std::this_thread::get_id());
}

inline void EnableInterrupt() { GetInterruptStatusRef() = true; }

inline void DisableInterrupt() { GetInterruptStatusRef() = false; }

inline bool GetInterruptStatus() { return GetInterruptStatusRef(); }

}  // namespace cpu_io

#endif /* SIMPLEKERNEL_TESTS_UNIT_TEST_CPU_IO_H_ */
