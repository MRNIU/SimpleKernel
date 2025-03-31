
/**
 * @file sk_libc.c
 * @brief c 运行时支持
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

#include "sk_libc.h"

#include <limits.h>
#include <stddef.h>
#include <stdint.h>

#include "sk_ctype.h"

#ifdef __cplusplus
extern "C" {
#endif

/// 栈保护
uint64_t __stack_chk_guard = 0x595E9FBD94FDA766;

/// 栈保护检查失败后进入死循环
__attribute__((noreturn)) void __stack_chk_fail() {
  while (true)
    ;
}

int atoi(const char* nptr) { return (int)strtol(nptr, NULL, 10); }

long int atol(const char* nptr) { return strtol(nptr, NULL, 10); }

long long int atoll(const char* nptr) { return strtoll(nptr, NULL, 10); }

long int strtol(const char* nptr, char** endptr, int base) {
  unsigned long long int result = strtoull(nptr, endptr, base);

  // Check if result will fit in a long int
  if (result > LONG_MAX) {
    return LONG_MAX;
  }

  return (long int)result;
}

long long int strtoll(const char* nptr, char** endptr, int base) {
  unsigned long long int result = strtoull(nptr, endptr, base);

  // Check if result will fit in a long long int
  if (result > LLONG_MAX) {
    return LLONG_MAX;
  }

  return (long long int)result;
}

unsigned long int strtoul(const char* nptr, char** endptr, int base) {
  unsigned long long int result = strtoull(nptr, endptr, base);

  // Check if result will fit in an unsigned long
  if (result > ULONG_MAX) {
    return ULONG_MAX;
  }

  return (unsigned long int)result;
}

/**
 * @brief Tables for efficient string-to-number conversions
 *
 * These tables store precomputed values to avoid expensive division operations
 * during string-to-number conversions.
 *
 * - __strtol_ul_max_tab[base] = ULONG_MAX / base
 *   The largest value that can be multiplied by base without overflow
 *
 * - __strtol_ul_rem_tab[base] = ULONG_MAX % base
 *   The maximum digit value that can be added after multiplication
 */
static const unsigned long __strtol_ul_max_tab[37] = {[0] = 0,  // Invalid base
                                                      [1] = 0,  // Invalid base
                                                      [2] = ULONG_MAX / 2,
                                                      [3] = ULONG_MAX / 3,
                                                      [4] = ULONG_MAX / 4,
                                                      [5] = ULONG_MAX / 5,
                                                      [6] = ULONG_MAX / 6,
                                                      [7] = ULONG_MAX / 7,
                                                      [8] = ULONG_MAX / 8,
                                                      [9] = ULONG_MAX / 9,
                                                      [10] = ULONG_MAX / 10,
                                                      [11] = ULONG_MAX / 11,
                                                      [12] = ULONG_MAX / 12,
                                                      [13] = ULONG_MAX / 13,
                                                      [14] = ULONG_MAX / 14,
                                                      [15] = ULONG_MAX / 15,
                                                      [16] = ULONG_MAX / 16,
                                                      [17] = ULONG_MAX / 17,
                                                      [18] = ULONG_MAX / 18,
                                                      [19] = ULONG_MAX / 19,
                                                      [20] = ULONG_MAX / 20,
                                                      [21] = ULONG_MAX / 21,
                                                      [22] = ULONG_MAX / 22,
                                                      [23] = ULONG_MAX / 23,
                                                      [24] = ULONG_MAX / 24,
                                                      [25] = ULONG_MAX / 25,
                                                      [26] = ULONG_MAX / 26,
                                                      [27] = ULONG_MAX / 27,
                                                      [28] = ULONG_MAX / 28,
                                                      [29] = ULONG_MAX / 29,
                                                      [30] = ULONG_MAX / 30,
                                                      [31] = ULONG_MAX / 31,
                                                      [32] = ULONG_MAX / 32,
                                                      [33] = ULONG_MAX / 33,
                                                      [34] = ULONG_MAX / 34,
                                                      [35] = ULONG_MAX / 35,
                                                      [36] = ULONG_MAX / 36};

static const unsigned long __strtol_ul_rem_tab[37] = {[0] = 0,  // Invalid base
                                                      [1] = 0,  // Invalid base
                                                      [2] = ULONG_MAX % 2,
                                                      [3] = ULONG_MAX % 3,
                                                      [4] = ULONG_MAX % 4,
                                                      [5] = ULONG_MAX % 5,
                                                      [6] = ULONG_MAX % 6,
                                                      [7] = ULONG_MAX % 7,
                                                      [8] = ULONG_MAX % 8,
                                                      [9] = ULONG_MAX % 9,
                                                      [10] = ULONG_MAX % 10,
                                                      [11] = ULONG_MAX % 11,
                                                      [12] = ULONG_MAX % 12,
                                                      [13] = ULONG_MAX % 13,
                                                      [14] = ULONG_MAX % 14,
                                                      [15] = ULONG_MAX % 15,
                                                      [16] = ULONG_MAX % 16,
                                                      [17] = ULONG_MAX % 17,
                                                      [18] = ULONG_MAX % 18,
                                                      [19] = ULONG_MAX % 19,
                                                      [20] = ULONG_MAX % 20,
                                                      [21] = ULONG_MAX % 21,
                                                      [22] = ULONG_MAX % 22,
                                                      [23] = ULONG_MAX % 23,
                                                      [24] = ULONG_MAX % 24,
                                                      [25] = ULONG_MAX % 25,
                                                      [26] = ULONG_MAX % 26,
                                                      [27] = ULONG_MAX % 27,
                                                      [28] = ULONG_MAX % 28,
                                                      [29] = ULONG_MAX % 29,
                                                      [30] = ULONG_MAX % 30,
                                                      [31] = ULONG_MAX % 31,
                                                      [32] = ULONG_MAX % 32,
                                                      [33] = ULONG_MAX % 33,
                                                      [34] = ULONG_MAX % 34,
                                                      [35] = ULONG_MAX % 35,
                                                      [36] = ULONG_MAX % 36};

unsigned long long int strtoull(const char* nptr, char** endptr, int base) {
  int negative;
  unsigned long long int cutoff;
  unsigned int cutlim;
  unsigned long long int i;
  const char* s;
  unsigned char c;
  const char* save;
  const char* end;
  int overflow;

  if (base < 0 || base == 1 || base > 36) {
    return 0;
  }

  save = s = nptr;

  /* Skip white space.  */
  while (isspace(*s) != 0) {
    ++s;
  }
  if ((*s == ('\0'))) {
    goto noconv;
  }

  /* Check for a sign.  */
  negative = 0;
  if (*s == ('-')) {
    negative = 1;
    ++s;
  } else if (*s == ('+')) {
    ++s;
  }

  /* Recognize number prefix and if BASE is zero, figure it out ourselves.  */
  if (*s == ('0')) {
    if ((base == 0 || base == 16) && toupper(s[1]) == ('X')) {
      s += 2;
      base = 16;
    } else if (base == 0) {
      base = 8;
    }
  } else if (base == 0) {
    base = 10;
  }

  /* Save the pointer so we can check later if anything happened.  */
  save = s;
  end = NULL;

  /* Avoid runtime division; lookup cutoff and limit.  */
  cutoff = __strtol_ul_max_tab[base - 2];
  cutlim = __strtol_ul_rem_tab[base - 2];

  overflow = 0;
  i = 0;
  c = *s;
  if (sizeof(long int) != sizeof(long long int)) {
    unsigned long int j = 0;
    unsigned long int jmax = __strtol_ul_max_tab[base - 2];

    for (; c != ('\0'); c = *++s) {
      if (s == end) {
        break;
      }
      if (c >= ('0') && c <= ('9')) {
        c -= ('0');
      } else if (isalpha(c) != 0) {
        c = toupper(c) - ('A') + 10;
      } else {
        break;
      }
      if ((int)c >= base) {
        break;
      }
      /* Note that we never can have an overflow.  */
      else if (j >= jmax) {
        /* We have an overflow.  Now use the long representation.  */
        i = (unsigned long long int)j;
        goto use_long;
      } else {
        j = j * (unsigned long int)base + c;
      }
    }

    i = (unsigned long long int)j;
  } else
    for (; c != ('\0'); c = *++s) {
      if (s == end) {
        break;
      }
      if (c >= ('0') && c <= ('9')) {
        c -= ('0');
      } else if (isalpha(c) != 0) {
        c = toupper(c) - ('A') + 10;
      } else {
        break;
      }
      if ((int)c >= base) {
        break;
      }
      /* Check for overflow.  */
      if (i > cutoff || (i == cutoff && c > cutlim)) {
        overflow = 1;
      } else {
      use_long:
        i *= (unsigned long long int)base;
        i += c;
      }
    }

  /* Check if anything actually happened.  */
  if (s == save) {
    goto noconv;
  }

  /* Store in ENDPTR the address of one character
     past the last character we converted.  */
  if (endptr != NULL) {
    *endptr = (char*)s;
  }

  if (overflow != 0) {
    return ULLONG_MAX;
  }

  /* Return the result of the appropriate sign.  */
  return negative ? -i : i;

noconv:
  /* We must handle a special case here: the base is 0 or 16 and the
     first two characters are '0' and 'x', but the rest are no
     hexadecimal digits.  Likewise when the base is 0 or 2 and the
     first two characters are '0' and 'b', but the rest are no binary
     digits.  This is no error case.  We return 0 and ENDPTR points to
     the 'x' or 'b'.  */
  if (endptr != NULL) {
    if (save - nptr >= 2 && (toupper(save[-1]) == ('X')) && save[-2] == ('0')) {
      *endptr = (char*)&save[-1];
    } else {
      /*  There was no number to convert.  */
      *endptr = (char*)nptr;
    }
  }

  return 0L;
}

#ifdef __cplusplus
}
#endif
