/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
#define SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_

#include <stdarg.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

extern void (*sk_putchar)(int c, void* ctx);

int sk_printf(const char* format, ...) __attribute__((format(printf, 1, 2)));

int sk_snprintf(char* buffer, size_t bufsz, const char* format, ...)
    __attribute__((format(printf, 3, 4)));

int sk_vsnprintf(char* buffer, size_t bufsz, const char* format, va_list vlist)
#ifdef __cplusplus
    noexcept
#endif
    __attribute__((format(printf, 3, 0)));

#ifdef __cplusplus
}
#endif

#endif /* SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_ */
