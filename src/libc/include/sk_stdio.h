/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
#define SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_

#ifdef __cplusplus
extern "C" {
#endif

// 底层字符输出原语，由各架构的 early_console.cpp 实现
void etl_putchar(int c);

// 输出 NUL 结尾的字符串，NULL 安全
void sk_print_str(const char* s);

// 输出无符号 64 位整数（十进制）
void sk_print_uint(unsigned long long v);

// 输出有符号 64 位整数（十进制）
void sk_print_int(long long v);

// 输出无符号 64 位整数（十六进制）
// width：最小输出位数，不足时用 '0' 补齐；upper 非零则用大写 A-F
void sk_print_hex(unsigned long long v, int width, int upper);

#ifdef __cplusplus
}
#endif

#endif  // SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
