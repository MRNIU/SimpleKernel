/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#pragma once

#include <sys/cdefs.h>

#include <cstddef>
#include <cstdint>

/// 启动 APs 的默认地址
inline constexpr uint64_t kDefaultAPBase = 0x30000;

extern "C" void* ap_start16[];
extern "C" void* ap_start64_end[];
extern "C" void* sipi_params[];

/**
 * @brief SIPI 参数结构体
 */
struct [[gnu::packed]] SipiParams {
  uint32_t cr3;
};
