/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_TESTS_UNIT_TEST_CPU_IO_H_
#define SIMPLEKERNEL_TESTS_UNIT_TEST_CPU_IO_H_

#include <cstddef>

namespace cpu_io {

// 测试环境中的空实现
inline void EnableInterrupt() {}
inline void DisableInterrupt() {}
inline bool GetInterruptStatus() { return false; }
inline size_t GetCurrentCoreId() { return 0; }

}  // namespace cpu_io

#endif /* SIMPLEKERNEL_TESTS_UNIT_TEST_CPU_IO_H_ */
