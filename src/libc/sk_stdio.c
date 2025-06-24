
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

#include "sk_stdio.h"

#ifdef __cplusplus
extern "C" {
#endif

#define NANOPRINTF_IMPLEMENTATION
#define NANOPRINTF_USE_FIELD_WIDTH_FORMAT_SPECIFIERS 1
#define NANOPRINTF_USE_PRECISION_FORMAT_SPECIFIERS 1
#define NANOPRINTF_USE_FLOAT_FORMAT_SPECIFIERS 0
#define NANOPRINTF_USE_SMALL_FORMAT_SPECIFIERS 1
#define NANOPRINTF_USE_LARGE_FORMAT_SPECIFIERS 1
#define NANOPRINTF_USE_BINARY_FORMAT_SPECIFIERS 1
#define NANOPRINTF_USE_WRITEBACK_FORMAT_SPECIFIERS 0
#define NANOPRINTF_USE_ALT_FORM_FLAG 1
#include <nanoprintf.h>

int sk_printf(const char* format, ...) {
  va_list args;
  va_start(args, format);
  const int ret = npf_vpprintf(sk_putchar, NULL, format, args);
  va_end(args);
  return ret;
}

int sk_snprintf(char* buffer, size_t bufsz, const char* format, ...) {
  va_list val;
  va_start(val, format);
  int const rv = npf_vsnprintf(buffer, bufsz, format, val);
  va_end(val);
  return rv;
}

int sk_vsnprintf(char* buffer, size_t bufsz, const char* format,
                 va_list vlist) {
  return npf_vsnprintf(buffer, bufsz, format, vlist);
}

#ifdef __cplusplus
}
#endif
