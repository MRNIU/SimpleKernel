/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"
#include "sk_libcxx.h"
#include "task.hpp"

namespace {

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  ArchInitSMP(argc, argv);
  MemoryInitSMP();
  InterruptInitSMP(argc, argv);
  klog::Info("Hello SimpleKernel SMP\n");
  return 0;
}

}  // namespace

void _start(int argc, const char** argv) {
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

auto main(int argc, const char** argv) -> int {
  // 架构相关初始化
  ArchInit(argc, argv);
  // 内存相关初始化
  MemoryInit();
  // 中断相关初始化
  InterruptInit(argc, argv);

  // 唤醒其余 core
  WakeUpOtherCores();

  // klog::Debug("Hello SimpleKernel\n");
  // klog::Info("Hello SimpleKernel\n");
  // klog::Warn("Hello SimpleKernel\n");
  // klog::Err("Hello SimpleKernel\n");

  DumpStack();

  TaskControlBlock("init", 1);

  klog::info << "Hello SimpleKernel\n";

  return 0;
}
