/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 内核入口
 */

#include <bmalloc.hpp>
#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"
#include "sk_libcxx.h"

namespace {

/// 非启动核入口
auto main_smp(int argc, const char **argv) -> int {
  ArchInitSMP(argc, argv);
  klog::Info("Hello SimpleKernel SMP\n");
  return 0;
}

}  // namespace

void _start(int argc, const char **argv) {
  if (argv != nullptr) {
    CppInit();
    main(argc, argv);
    CppDeInit();
  } else {
    main_smp(argc, argv);
  }

  // 进入死循环
  while (true) {
    ;
  }
}

// 日志函数类型
struct TestLogger {
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

auto main(int argc, const char **argv) -> int {
  // 架构相关初始化
  ArchInit(argc, argv);

  // klog::Debug("Hello SimpleKernel\n");
  // klog::Info("Hello SimpleKernel\n");
  // klog::Warn("Hello SimpleKernel\n");
  // klog::Err("Hello SimpleKernel\n");

  DumpStack();

  void *allocator_addr = reinterpret_cast<void *>(
      (reinterpret_cast<uintptr_t>(end) + 4095) & ~4095);
  size_t allocator_size =
      Singleton<BasicInfo>::GetInstance().physical_memory_size -
      reinterpret_cast<uintptr_t>(allocator_addr) +
      Singleton<BasicInfo>::GetInstance().physical_memory_addr;

  klog::Info("bmalloc address: %p\n", allocator_addr);
  klog::Info("bmalloc size: %zu\n", allocator_size);

  bmalloc::Bmalloc<TestLogger> allocator(allocator_addr, allocator_size);

  klog::info << "Hello SimpleKernel\n";

  return 0;
}
