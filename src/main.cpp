/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <MPMCQueue.hpp>
#include <cstdint>
#include <new>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt.h"
#include "kernel.h"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
#include "sk_cstdio"
#include "sk_cstring"
#include "sk_iostream"
#include "sk_libcxx.h"
#include "sk_list"
#include "sk_priority_queue"
#include "sk_stdlib.h"
#include "sk_vector"
#include "syscall.hpp"
#include "virtual_memory.hpp"

namespace {

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  per_cpu::GetCurrentCore() = per_cpu::PerCpu(cpu_io::GetCurrentCoreId());
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
  } else {
    main_smp(argc, argv);
  }

  while (1)
    ;
}

auto main(int argc, const char** argv) -> int {
  // 初始化当前核心的 per_cpu 数据
  per_cpu::GetCurrentCore() = per_cpu::PerCpu(cpu_io::GetCurrentCoreId());

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

  return 0;
}
