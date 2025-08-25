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
    klog::Info("%s", buffer);
    return result;
  }
};

}  // namespace

static bmalloc::Bmalloc<BmallocLogger> *allocator = nullptr;

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
  void *allocator_addr = reinterpret_cast<void *>(
      (reinterpret_cast<uintptr_t>(end) + 4095) & ~4095);
  size_t allocator_size =
      Singleton<BasicInfo>::GetInstance().physical_memory_size -
      reinterpret_cast<uintptr_t>(allocator_addr) +
      Singleton<BasicInfo>::GetInstance().physical_memory_addr;

  klog::Info("bmalloc address: %p\n", allocator_addr);
  klog::Info("bmalloc size: %zu\n", allocator_size);

  static bmalloc::Bmalloc<BmallocLogger> bmallocator(allocator_addr,
                                                     allocator_size);
  allocator = &bmallocator;

  std::array<size_t, 13> sizes = {1,   2,   4,   8,    16,   32,  64,
                                  128, 256, 512, 1024, 2048, 4096};

  for (size_t size : sizes) {
    void *ptr = malloc(size);
    if (ptr != nullptr) {
      // 写入边界位置
      char *bytes = static_cast<char *>(ptr);
      bytes[0] = 0xAA;
      bytes[size - 1] = 0xBB;

      // 验证写入
      if (memcmp(&bytes[0], "\xAA", 1) != 0) {
        klog::Err("Memory verification failed for size %zu at start\n", size);
      }
      if (memcmp(&bytes[size - 1], "\xBB", 1) != 0) {
        klog::Err("Memory verification failed for size %zu at end\n", size);
      }

      free(ptr);
    }
  }
  klog::Info("Hello MemoryInit\n");
}

void MemoryInitSMP() { klog::Info("Hello MemoryInitSMP\n"); }
