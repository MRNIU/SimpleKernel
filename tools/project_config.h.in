/**
 * @copyright Copyright The SimpleKernel Contributors
 * @note 此文件由 cmake 自动生成，不要手动修改
 */

#ifndef SIMPLEKERNEL_SRC_PROJECT_CONFIG_H_
#define SIMPLEKERNEL_SRC_PROJECT_CONFIG_H_

#include <cstdint>

#ifdef SIMPLEKERNEL_EARLY_CONSOLE
static constexpr const uint64_t kSimpleKernelEarlyConsoleBase =
    SIMPLEKERNEL_EARLY_CONSOLE;
#else
static constexpr const uint64_t kSimpleKernelEarlyConsoleBase = 0x0;
#endif

#ifdef SIMPLEKERNEL_MAX_CORE_COUNT
static constexpr const uint64_t kSimpleKernelMaxCoreCount =
    SIMPLEKERNEL_MAX_CORE_COUNT;
#else
static constexpr const uint64_t kSimpleKernelMaxCoreCount = 4;
#endif

#ifdef SIMPLEKERNEL_PER_CPU_ALIGN_SIZE
static constexpr const uint64_t kSimpleKernelPerCpuAlignSize =
    SIMPLEKERNEL_PER_CPU_ALIGN_SIZE;
#else
static constexpr const uint64_t kSimpleKernelPerCpuAlignSize = 128;
#endif

#ifdef SIMPLEKERNEL_DEFAULT_STACK_SIZE
static constexpr const uint64_t kSimpleKernelDefaultStackSize =
    SIMPLEKERNEL_DEFAULT_STACK_SIZE;
#else
static constexpr const uint64_t kSimpleKernelDefaultStackSize = 4096;
#endif

#endif /* SIMPLEKERNEL_SRC_PROJECT_CONFIG_H_ */
