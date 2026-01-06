/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief project_config 头文件，此文件由 cmake 自动生成，不要手动修改
 */

#ifndef SIMPLEKERNEL_SRC_PROJECT_CONFIG_H_
#define SIMPLEKERNEL_SRC_PROJECT_CONFIG_H_

#include <cstdint>

#ifdef SIMPLEKERNEL_DEBUG
static constexpr const auto kSimpleKernelDebugLog = true;
#else
static constexpr const auto kSimpleKernelDebugLog = false;
#endif

#ifdef SIMPLEKERNEL_EARLY_CONSOLE
static constexpr const uint64_t kSimpleKernelEarlyConsoleBase =
    SIMPLEKERNEL_EARLY_CONSOLE;
#else
static constexpr const uint64_t kSimpleKernelEarlyConsoleBase = 0x0;
#endif

#endif /* SIMPLEKERNEL_SRC_PROJECT_CONFIG_H_ */
