/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_stdio.h"

#ifdef __cplusplus
extern "C" {
#endif

// 输出 NUL 结尾的字符串，NULL 安全
void sk_print_str(const char* s) {
  if (!s) {
    s = "(null)";
  }
  while (*s) {
    etl_putchar((unsigned char)*s);
    ++s;
  }
}

// 输出无符号 64 位整数（十进制）
void sk_print_uint(unsigned long long v) {
  char buf[20];
  int n = 0;
  if (v == 0) {
    etl_putchar('0');
    return;
  }
  while (v) {
    buf[n++] = (char)('0' + (int)(v % 10));
    v /= 10;
  }
  for (int i = n - 1; i >= 0; i--) {
    etl_putchar(buf[i]);
  }
}

// 输出有符号 64 位整数（十进制）
void sk_print_int(long long v) {
  if (v < 0) {
    etl_putchar('-');
    sk_print_uint(-(unsigned long long)v);
  } else {
    sk_print_uint((unsigned long long)v);
  }
}

// 输出无符号 64 位整数（十六进制）
// width：最小输出位数，不足时用 '0' 补齐；upper 非零则用大写 A-F
void sk_print_hex(unsigned long long v, int width, int upper) {
  static const char kLo[] = "0123456789abcdef";
  static const char kHi[] = "0123456789ABCDEF";
  const char* digits = upper ? kHi : kLo;
  char buf[16];
  int n = 0;
  if (v == 0) {
    buf[n++] = '0';
  } else {
    while (v) {
      buf[n++] = digits[v & 0xf];
      v >>= 4;
    }
  }
  for (int i = n; i < width; i++) {
    etl_putchar('0');
  }
  for (int i = n - 1; i >= 0; i--) {
    etl_putchar(buf[i]);
  }
}

#ifdef __cplusplus
}
#endif
