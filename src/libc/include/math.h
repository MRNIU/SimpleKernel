/**
 * @copyright Copyright The SimpleKernel Contributors
 */

/**
 * @file math.h
 * @brief Freestanding shim for <math.h>
 *
 * In GCC 14+ freestanding builds, the C++ stdlib wrapper
 * /usr/riscv64-linux-gnu/include/c++/14/math.h unconditionally includes
 * <cmath>, which triggers bits/requires_hosted.h (#error in freestanding
 * mode). This shim intercepts the include before the wrapper is reached,
 * temporarily enables _GLIBCXX_INCLUDE_NEXT_C_HEADERS so the wrapper
 * passes through to the actual C-level math.h, then restores the state.
 *
 * This file must reside in an -I directory that precedes the C++ stdlib
 * wrapper paths in the compiler's include search order.
 */

#pragma GCC system_header

#ifdef __cplusplus
#define _GLIBCXX_INCLUDE_NEXT_C_HEADERS
#endif

#include_next <math.h>  // NOLINT: intentional include_next for wrapper bypass

#ifdef __cplusplus
#undef _GLIBCXX_INCLUDE_NEXT_C_HEADERS
#endif
