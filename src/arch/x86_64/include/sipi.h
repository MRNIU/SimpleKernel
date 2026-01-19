/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_ARCH_X86_64_SIPI_H_
#define SIMPLEKERNEL_SRC_ARCH_X86_64_SIPI_H_

#include <sys/cdefs.h>

#include <cstddef>
#include <cstdint>

/// 启动 APs 的默认地址
static constexpr uint64_t kDefaultAPBase = 0x30000;

extern "C" void *ap_start16[];
extern "C" void *ap_start64_end[];
extern "C" void *sipi_params[];

struct sipi_params_t {
  uint32_t cr3;
} __attribute__((packed));

#endif /* SIMPLEKERNEL_SRC_ARCH_X86_64_SIPI_H_ */
