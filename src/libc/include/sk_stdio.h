/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
#define SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_

#include "stb_sprintf.h"

#ifdef __cplusplus
extern "C" {
#endif

// 底层字符输出原语，由各架构的 early_console.cpp 实现
void etl_putchar(int c);

#ifdef __cplusplus
}
#endif

#endif  // SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
