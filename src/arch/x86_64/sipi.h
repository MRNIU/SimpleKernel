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
extern "C" void *sipi_params_16bit[];
extern "C" void *sipi_params[];

/**
 * 与 boot.S 中的 sipi_params_16bit 结构对应，传递给 ap_start16 的参数
 */
struct sipi_params_16bit_t {
  uint32_t ap_start;
  uint16_t segment;
  uint16_t pad;
  uint16_t gdt_limit;
  uint32_t gdt;
  uint16_t unused;
} __attribute__((packed));

// struct sipi_params {
//   uint32_t idt_ptr;
//   uint32_t stack_top;
//   uint32_t stack_size;
//   uint32_t microcode_lock;
//   uint32_t microcode_ptr;
//   uint32_t msr_table_ptr;
//   uint32_t msr_count;
//   uint32_t c_handler;
//   atomic_t ap_count;
// };

#endif /* SIMPLEKERNEL_SRC_ARCH_X86_64_SIPI_H_ */
