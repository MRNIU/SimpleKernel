/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
#define SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Output primitive — implemented per-arch in early_console.cpp */
void etl_putchar(int c);

/* ── Freestanding emit helpers (always-inline, no format strings) ────────── */

/** Emit a NUL-terminated C string character by character. NULL-safe. */
static __attribute__((always_inline)) inline void sk_emit_str(const char* s) {
  if (!s) {
    s = "(null)";
  }
  while (*s) {
    etl_putchar((unsigned char)*s);
    ++s;
  }
}

/** Emit a signed 64-bit integer in decimal. */
static __attribute__((always_inline)) inline void sk_emit_sint(long long v) {
  char buf[20];
  int n = 0;
  if (v < 0) {
    etl_putchar('-');
    /* avoid UB on LLONG_MIN: cast before negation */
    unsigned long long uv = (unsigned long long)(-(v + 1)) + 1ULL;
    if (uv == 0) {
      etl_putchar('0');
      return;
    }
    while (uv) {
      buf[n++] = (char)('0' + (int)(uv % 10));
      uv /= 10;
    }
  } else {
    unsigned long long uv = (unsigned long long)v;
    if (uv == 0) {
      etl_putchar('0');
      return;
    }
    while (uv) {
      buf[n++] = (char)('0' + (int)(uv % 10));
      uv /= 10;
    }
  }
  for (int i = n - 1; i >= 0; i--) etl_putchar(buf[i]);
}

/** Emit an unsigned 64-bit integer in decimal. */
static __attribute__((always_inline)) inline void sk_emit_uint(
    unsigned long long v) {
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
  for (int i = n - 1; i >= 0; i--) etl_putchar(buf[i]);
}

/**
 * Emit an unsigned 64-bit integer in hexadecimal.
 * @param v     Value to emit.
 * @param width Minimum digit count (zero-padded with '0').
 * @param upper Non-zero → uppercase A-F.
 */
static __attribute__((always_inline)) inline void sk_emit_hex(
    unsigned long long v, int width, int upper) {
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
  for (int i = n; i < width; i++) etl_putchar('0');
  for (int i = n - 1; i >= 0; i--) etl_putchar(buf[i]);
}

#ifdef __cplusplus
}
#endif

#endif  // SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
