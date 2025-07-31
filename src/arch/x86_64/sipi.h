/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief sipi 头文件
 */

#ifndef SIMPLEKERNEL_SRC_ARCH_X86_64_SIPI_H_
#define SIMPLEKERNEL_SRC_ARCH_X86_64_SIPI_H_

#include <sys/cdefs.h>

#include <cstddef>
#include <cstdint>

/// 启动 APs 的默认地址
static constexpr uint64_t kDefaultAPBase = 0x30000;

#endif /* SIMPLEKERNEL_SRC_ARCH_X86_64_SIPI_H_ */
