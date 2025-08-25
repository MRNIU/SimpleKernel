/**
 * Copyright The SimpleKernel Contributors
 */

#include <bmalloc.hpp>
#include <cstddef>

#include "basic_info.hpp"
#include "kernel_log.hpp"
#include "virtual_memory.hpp"

namespace {

struct BmallocLogger {
  int operator()(const char *format, ...) const {
    va_list args;
    va_start(args, format);
    char buffer[1024];
    int result = sk_vsnprintf(buffer, sizeof(buffer), format, args);
    va_end(args);
    klog::Err("%s", buffer);
    return result;
  }
};

static bmalloc::Bmalloc<BmallocLogger> *allocator = nullptr;
}  // namespace

extern "C" void *malloc(size_t size) {
  if (allocator) {
    return allocator->malloc(size);
  }
  return nullptr;
}

extern "C" void *aligned_alloc(size_t alignment, size_t size) {
  if (allocator) {
    return allocator->aligned_alloc(alignment, size);
  }
  return nullptr;
}
extern "C" void free(void *ptr) {
  if (allocator) {
    allocator->free(ptr);
  }
}

void MemoryInit() {
  auto allocator_addr = reinterpret_cast<void *>(
      (reinterpret_cast<uint64_t>(end) + 4095) & ~4095);
  auto allocator_size =
      Singleton<BasicInfo>::GetInstance().physical_memory_addr +
      Singleton<BasicInfo>::GetInstance().physical_memory_size -
      reinterpret_cast<uint64_t>(allocator_addr);

  klog::Info("bmalloc address: %p\n", allocator_addr);
  klog::Info("bmalloc size: %zu\n", allocator_size);

  static bmalloc::Bmalloc<BmallocLogger> bmallocator(allocator_addr,
                                                     allocator_size);
  allocator = &bmallocator;

  klog::Info("Hello MemoryInit\n");
}

void MemoryInitSMP() { klog::Info("Hello MemoryInitSMP\n"); }
