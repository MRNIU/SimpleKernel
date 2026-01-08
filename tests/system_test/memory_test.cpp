/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <cstddef>
#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "sk_cstdio"
#include "sk_cstring"
#include "sk_libc.h"
#include "sk_libcxx.h"
#include "system_test.h"

extern "C" {
void *malloc(size_t size);
void free(void *ptr);
void *aligned_alloc(size_t alignment, size_t size);
}

auto memory_test() -> bool {
  sk_printf("memory_test: start\n");

  // Test 1: malloc & free
  size_t size = 1024;
  void *ptr = malloc(size);
  if (ptr == nullptr) {
    sk_printf("memory_test: malloc failed\n");
    return false;
  }

  // Write and read verification
  auto *byte_ptr = static_cast<uint8_t *>(ptr);
  for (size_t i = 0; i < size; ++i) {
    byte_ptr[i] = static_cast<uint8_t>(i & 0xFF);
  }

  for (size_t i = 0; i < size; ++i) {
    if (byte_ptr[i] != static_cast<uint8_t>(i & 0xFF)) {
      sk_printf("memory_test: verify failed at %lu\n", i);
      free(ptr);
      return false;
    }
  }

  free(ptr);
  sk_printf("memory_test: malloc/free passed\n");

  // Test 2: aligned_alloc
  size_t alignment = 256;
  size_t aligned_size = 512;
  void *aligned_ptr = aligned_alloc(alignment, aligned_size);
  if (aligned_ptr == nullptr) {
    sk_printf("memory_test: aligned_alloc failed\n");
    return false;
  }

  if ((reinterpret_cast<uintptr_t>(aligned_ptr) & (alignment - 1)) != 0) {
    sk_printf("memory_test: aligned_alloc alignment failed: %p\n", aligned_ptr);
    free(aligned_ptr);
    return false;
  }

  free(aligned_ptr);
  sk_printf("memory_test: aligned_alloc passed\n");

  // Test 3: Multiple small allocations
  const int count = 10;
  void *ptrs[count];
  bool alloc_failed = false;
  for (int i = 0; i < count; ++i) {
    ptrs[i] = malloc(128);
    if (ptrs[i] == nullptr) {
      sk_printf("memory_test: multi alloc failed at %d\n", i);
      alloc_failed = true;
      // Free previous
      for (int j = 0; j < i; ++j) {
        free(ptrs[j]);
      }
      break;
    }
    // Fill
    sk_std::memset(ptrs[i], i, 128);
  }
  if (alloc_failed) {
    return false;
  }

  for (int i = 0; i < count; ++i) {
    auto *p = static_cast<uint8_t *>(ptrs[i]);
    for (int j = 0; j < 128; ++j) {
      if (p[j] != i) {
        sk_printf("memory_test: multi alloc verify failed at %d\n", i);
        // Cleanup
        for (int k = 0; k < count; ++k) {
          free(ptrs[k]);
        }
        return false;
      }
    }
    free(ptrs[i]);
  }
  sk_printf("memory_test: multi alloc passed\n");

  return true;
}
