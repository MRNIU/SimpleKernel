/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 内存分配实现
 */

#include "sk_new"

#include <cstddef>
#include <cstring>

extern "C" {
void* malloc(size_t size);
void* aligned_alloc(size_t alignment, size_t size);
void free(void* ptr);
}

void* operator new(size_t size) noexcept {
  if (size == 0) {
    size = 1;
  }
  return malloc(size);
}

void* operator new[](size_t size) noexcept {
  if (size == 0) {
    size = 1;
  }
  return malloc(size);
}

void* operator new(size_t size, size_t alignment) noexcept {
  if (size == 0) {
    size = 1;
  }

  // 确保对齐参数是 2 的幂
  if (alignment == 0 || (alignment & (alignment - 1)) != 0) {
    return nullptr;
  }

  // 如果对齐要求小于等于默认对齐，使用普通 malloc
  if (alignment <= alignof(std::max_align_t)) {
    return malloc(size);
  }

  return aligned_alloc(alignment, size);
}

void* operator new[](size_t size, size_t alignment) noexcept {
  if (size == 0) {
    size = 1;
  }

  // 确保对齐参数是 2 的幂
  if (alignment == 0 || (alignment & (alignment - 1)) != 0) {
    return nullptr;
  }

  // 如果对齐要求小于等于默认对齐，使用普通 malloc
  if (alignment <= alignof(std::max_align_t)) {
    return malloc(size);
  }

  return aligned_alloc(alignment, size);
}

void operator delete(void* ptr) {
  if (ptr != nullptr) {
    free(ptr);
  }
}

void operator delete(void* ptr, size_t) {
  if (ptr != nullptr) {
    free(ptr);
  }
}

void operator delete[](void* ptr) {
  if (ptr != nullptr) {
    free(ptr);
  }
}

void operator delete[](void* ptr, size_t) {
  if (ptr != nullptr) {
    free(ptr);
  }
}

void operator delete(void* ptr, size_t, size_t) {
  if (ptr != nullptr) {
    free(ptr);
  }
}

void operator delete[](void* ptr, size_t, size_t) {
  if (ptr != nullptr) {
    free(ptr);
  }
}
