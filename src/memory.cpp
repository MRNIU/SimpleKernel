/**
 * Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <algorithm>
#include <bmalloc.hpp>
#include <cstddef>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "sk_stdlib.h"
#include "virtual_memory.hpp"

namespace {

struct BmallocLogger {
  int operator()(const char* format, ...) const {
    va_list args;
    va_start(args, format);
    char buffer[1024];
    int result = sk_vsnprintf(buffer, sizeof(buffer), format, args);
    va_end(args);
    klog::Err("%s", buffer);
    return result;
  }
};

bmalloc::Bmalloc<BmallocLogger>* allocator = nullptr;
}  // namespace

extern "C" void* malloc(size_t size) {
  if (allocator) {
    return allocator->malloc(size);
  }
  return nullptr;
}

extern "C" void free(void* ptr) {
  if (allocator) {
    allocator->free(ptr);
  }
}

extern "C" void* calloc(size_t num, size_t size) {
  if (allocator) {
    return allocator->calloc(num, size);
  }
  return nullptr;
}

extern "C" void* realloc(void* ptr, size_t new_size) {
  if (allocator) {
    return allocator->realloc(ptr, new_size);
  }
  return nullptr;
}

extern "C" void* aligned_alloc(size_t alignment, size_t size) {
  if (allocator) {
    return allocator->aligned_alloc(alignment, size);
  }
  return nullptr;
}

extern "C" void aligned_free(void* ptr) {
  if (allocator) {
    allocator->aligned_free(ptr);
  }
}

void MemoryInit() {
  auto allocator_addr =
      reinterpret_cast<void*>(cpu_io::virtual_memory::PageAlignUp(
          BasicInfoSingleton::instance().elf_addr +
          KernelElfSingleton::instance().GetElfSize()));
  auto allocator_size = BasicInfoSingleton::instance().physical_memory_addr +
                        BasicInfoSingleton::instance().physical_memory_size -
                        reinterpret_cast<uint64_t>(allocator_addr);

  klog::Info("bmalloc address: %p, size: 0x%lX\n", allocator_addr,
             allocator_size);

  static bmalloc::Bmalloc<BmallocLogger> bmallocator(allocator_addr,
                                                     allocator_size);
  allocator = &bmallocator;

  // 初始化当前核心的虚拟内存
  VirtualMemorySingleton::create();
  VirtualMemorySingleton::instance().InitCurrentCore();

  // 重新映射早期控制台地址（如果有的话）
  if (SIMPLEKERNEL_EARLY_CONSOLE_BASE != 0) {
    VirtualMemorySingleton::instance().MapMMIO(
        SIMPLEKERNEL_EARLY_CONSOLE_BASE, cpu_io::virtual_memory::kPageSize);
  }

  klog::Info("Memory initialization completed\n");
}

void MemoryInitSMP() {
  VirtualMemorySingleton::instance().InitCurrentCore();
  klog::Info("SMP Memory initialization completed\n");
}
