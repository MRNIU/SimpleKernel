/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>
#include <opensbi_interface.h>

#include <bmalloc.hpp>

#include "basic_info.hpp"
#include "config.h"
#include "kernel_elf.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "ns16550a.h"
#include "per_cpu.hpp"
#include "sk_cstdio"
#include "sk_libc.h"

namespace {

struct BmallocLogger {
  int operator()(const char* format, ...) const {
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

// 基本输出实现
extern "C" void sk_putchar(int c, [[maybe_unused]] void* ctx) {
  sbi_debug_console_write_byte(c);
}

BasicInfo::BasicInfo(int, const char** argv) {
  auto [memory_base, memory_size] =
      Singleton<KernelFdt>::GetInstance().GetMemory();
  physical_memory_addr = memory_base;
  physical_memory_size = memory_size;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);
  elf_addr = kernel_addr;
  elf_size = kernel_size;

  fdt_addr = reinterpret_cast<uint64_t>(argv);

  core_count = Singleton<KernelFdt>::GetInstance().GetCoreCount();
}

void ArchInit(int argc, const char** argv) {
  Singleton<KernelFdt>::GetInstance() =
      KernelFdt(reinterpret_cast<uint64_t>(argv));

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr);

  void* allocator_addr = reinterpret_cast<void*>(
      (reinterpret_cast<uintptr_t>(end) + 4095) & ~4095);
  size_t allocator_size =
      Singleton<BasicInfo>::GetInstance().physical_memory_size -
      reinterpret_cast<uintptr_t>(allocator_addr) +
      Singleton<BasicInfo>::GetInstance().physical_memory_addr;

  klog::Info("bmalloc address: %p\n", allocator_addr);
  klog::Info("bmalloc size: %zu\n", allocator_size);

  bmalloc::Bmalloc<BmallocLogger> allocator(allocator_addr, allocator_size);

  std::array<size_t, 13> sizes = {1,   2,   4,   8,    16,   32,  64,
                                  128, 256, 512, 1024, 2048, 4096};

  for (size_t size : sizes) {
    void* ptr = allocator.malloc(size);
    if (ptr != nullptr) {
      // 写入边界位置
      char* bytes = static_cast<char*>(ptr);
      bytes[0] = 0xAA;
      bytes[size - 1] = 0xBB;

      // 验证写入
      if (memcmp(&bytes[0], "\xAA", 1) != 0) {
        klog::Err("Memory verification failed for size %zu at start\n", size);
      }
      if (memcmp(&bytes[size - 1], "\xBB", 1) != 0) {
        klog::Err("Memory verification failed for size %zu at end\n", size);
      }

      allocator.free(ptr);
    }
  }

  klog::Info("Hello riscv64 ArchInit\n");
}

void ArchInitSMP(int, const char**) {}

void WakeUpOtherCores() {
  for (size_t i = 0; i < Singleton<BasicInfo>::GetInstance().core_count; i++) {
    auto ret = sbi_hart_start(i, reinterpret_cast<uint64_t>(_boot), 0);
    if ((ret.error != SBI_SUCCESS) &&
        (ret.error != SBI_ERR_ALREADY_AVAILABLE)) {
      klog::Warn("hart %d start failed: %d\n", i, ret.error);
    }
  }
}
