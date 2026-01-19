/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <MPMCQueue.hpp>
#include <cstdint>
#include <new>

#include "arch.h"
#include "arch/riscv64/interrupt.h"
#include "basic_info.hpp"
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
#include "task_control_block.hpp"
#include "task_manager.hpp"
#include "virtual_memory.hpp"

namespace {

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  per_cpu::GetCurrentCore() = per_cpu::PerCpu(cpu_io::GetCurrentCoreId());
  ArchInitSMP(argc, argv);
  MemoryInitSMP();
  InterruptInitSMP(argc, argv);
  Singleton<TaskManager>::GetInstance().InitCurrentCore();
  klog::Info("Hello SimpleKernel SMP\n");

  // 启动调度器
  Singleton<TaskManager>::GetInstance().Schedule();

  // 不应该执行到这里
  __builtin_unreachable();
}

}  // namespace

void _start(int argc, const char** argv) {
  if (argv != nullptr) {
    CppInit();
    main(argc, argv);
  } else {
    main_smp(argc, argv);
  }

  // 不应该执行到这里
  __builtin_unreachable();
}

void thread_func_a(void* arg) {
  while (1) {
    klog::Info("Thread A: running, arg=%d\n", (uint64_t)arg);
    // sys_sleep(100);
  }
}

void thread_func_b(void* arg) {
  while (1) {
    klog::Info("Thread B: running, arg=%d\n", (uint64_t)arg);
    // sys_sleep(100);
    sys_exit(233);
  }
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
  // 初始化任务管理器 (设置主线程)
  Singleton<TaskManager>::GetInstance().InitCurrentCore();

  // 唤醒其余 core
  // WakeUpOtherCores();

  DumpStack();

  klog::info << "Hello SimpleKernel\n";

  auto task_a = new TaskControlBlock(
      "Task A", Singleton<TaskManager>::GetInstance().AllocatePid(),
      thread_func_a, (void*)100);
  auto task_b = new TaskControlBlock(
      "Task B", Singleton<TaskManager>::GetInstance().AllocatePid(),
      thread_func_b, (void*)200);
  Singleton<TaskManager>::GetInstance().AddTask(task_a);
  Singleton<TaskManager>::GetInstance().AddTask(task_b);

  klog::Info("Main: Starting scheduler...\n");

  // 启动调度器，不再返回
  // 从这里开始，系统将在各个任务之间切换
  Singleton<TaskManager>::GetInstance().Schedule();

  // 不应该执行到这里
  __builtin_unreachable();
}
