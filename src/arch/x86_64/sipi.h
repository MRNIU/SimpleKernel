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

extern "C" void *ap_start16[];
extern "C" void *ap_start16_end[];
extern "C" void *ap_start[];
extern "C" void *sipi_params[];

struct sipi_params_t {
  uint32_t idt_ptr;
  uint32_t stack_top;
  uint32_t stack_size;
  uint32_t msr_table_ptr;
  uint32_t msr_count;
  uint32_t c_handler;
  volatile uint32_t ap_count;
} __attribute__((packed));

#endif /* SIMPLEKERNEL_SRC_ARCH_X86_64_SIPI_H_ */
