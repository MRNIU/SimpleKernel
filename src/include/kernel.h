/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_H_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_H_

#include <cstdint>

/**
 * @brief 负责 crtbegin 的工作
 * @param argc
 *          riscv64: 启动核 id
 * @param argv 参数指针
 *          riscv64: dtb 地址
 * @return uint32_t 正常返回 0
 */
extern "C" [[maybe_unused]] [[noreturn]] void _start(int argc,
                                                     const char** argv);

/**
 * @brief 内核入口
 * @param argc 同 _start
 * @param argv 同 _start
 * @return int 正常返回 0
 */
auto main(int argc, const char** argv) -> int;

void MemoryInit();
void MemoryInitSMP();

void DeviceInit();

void FileSystemInit();

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_H_
