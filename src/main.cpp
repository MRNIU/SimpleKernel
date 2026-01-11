/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstdint>

#include "arch.h"
#include "arch/riscv64/interrupt.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_cstring"
#include "sk_iostream"
#include "sk_libcxx.h"
#include "sk_list"
#include "sk_new"
#include "sk_stdlib.h"
#include "sk_vector"
#include "syscall.hpp"
#include "task.hpp"
#include "virtual_memory.hpp"

namespace {

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  ArchInitSMP(argc, argv);
  MemoryInitSMP();
  InterruptInitSMP(argc, argv);
  klog::Info("Hello SimpleKernel SMP\n");

  Singleton<TaskManager>::GetInstance().InitCurrentCore();

  sys_yield();

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

  DumpStack();

  klog::info << "Hello SimpleKernel\n";

  // 初始化任务管理器 (设置主线程)
  Singleton<TaskManager>::GetInstance().InitCurrentCore();

  sys_yield();

  return 0;
}
