/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_stdio.h"

#ifdef __cplusplus
extern "C" {
#endif

static void dummy_putchar(int c, void* ctx) {
  (void)c;
  (void)ctx;
}

void (*sk_putchar)(int, void*) = dummy_putchar;

#ifdef __cplusplus
}
#endif
