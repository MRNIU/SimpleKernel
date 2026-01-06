/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_ARCH_ARCH_H_
#define SIMPLEKERNEL_SRC_ARCH_ARCH_H_

#include <sys/cdefs.h>

#include <cstddef>
#include <cstdint>

/**
 * 体系结构相关初始化
 * @param argc 在不同体系结构有不同含义，同 _start
 * @param argv 在不同体系结构有不同含义，同 _start
 */
void ArchInit(int argc, const char** argv);
void ArchInitSMP(int argc, const char** argv);

/**
 * @brief 重新映射架构相关内存区域
 */
void ArchReMap();

/**
 * 唤醒其余 core
 */
void WakeUpOtherCores();

/**
 * 体系结构相关中断初始化
 * @param argc 在不同体系结构有不同含义，同 _start
 * @param argv 在不同体系结构有不同含义，同 _start
 */
void InterruptInit(int argc, const char** argv);
void InterruptInitSMP(int argc, const char** argv);

/// 最多回溯 128 层调用栈
static constexpr const size_t kMaxFrameCount = 128;

/**
 * 获取调用栈
 * @param buffer 指向一个数组，该数组用于存储调用栈中的返回地址
 * @param size 数组的大小，即调用栈中最多存储多少个返回地址
 * @return int 成功时返回实际写入数组中的地址数量，失败时返回 -1
 */
__always_inline auto backtrace(void** buffer, int size) -> int;

/**
 * 打印调用栈
 */
void DumpStack();

#endif /* SIMPLEKERNEL_SRC_ARCH_ARCH_H_ */
