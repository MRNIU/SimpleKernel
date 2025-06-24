
/**
 * @file sk_stdio.h
 * @brief sk_stdio 定义
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-03-31
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-03-31<td>Zone.N<td>迁移到 doxygen
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
#define SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_

#include <stdarg.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

extern void sk_putchar(int c, [[maybe_unused]] void* ctx);

int sk_printf(const char* format, ...) __attribute__((format(printf, 1, 2)));

int sk_snprintf(char* buffer, size_t bufsz, const char* format, ...)
    __attribute__((format(printf, 3, 4)));

int sk_vsnprintf(char* buffer, size_t bufsz, const char* format, va_list vlist)
    __attribute__((format(printf, 3, 0)));

#ifdef __cplusplus
}
#endif

#endif /* SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_ */
