/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <MPMCQueue.hpp>
#include <cerrno>
#include <cstdint>
#include <new>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt.h"
#include "kernel.h"
#include "kernel_log.hpp"
#include "mutex.hpp"
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

/// 全局变量，用于测试多核同步
std::atomic<uint64_t> global_counter{0};

/// Task1: 每 1s 打印一次，测试 sys_exit
void task1_func(void* arg) {
  klog::Info("Task1: arg = %p\n", arg);
  for (int i = 0; i < 5; ++i) {
    klog::Info("Task1: iteration %d/5\n", i + 1);
    sys_sleep(1000);
  }
  klog::Info("Task1: exiting with code 0\n");
  sys_exit(0);
}

/// Task2: 每 2s 打印一次，测试 sys_yield
void task2_func(void* arg) {
  klog::Info("Task2: arg = %p\n", arg);
  uint64_t count = 0;
  while (1) {
    klog::Info("Task2: yield count=%llu\n", count++);
    sys_sleep(2000);
    // 主动让出 CPU
    sys_yield();
  }
}

/// Task3: 每 3s 打印一次，同时修改全局变量，测试多核同步
void task3_func(void* arg) {
  klog::Info("Task3: arg = %p\n", arg);
  while (1) {
    uint64_t old_value = global_counter.fetch_add(1, std::memory_order_seq_cst);
    klog::Info("Task3: global_counter %llu -> %llu\n", old_value,
               old_value + 1);
    sys_sleep(3000);
  }
}

/// Task4: 每 4s 打印一次，测试 sys_sleep
void task4_func(void* arg) {
  klog::Info("Task4: arg = %p\n", arg);
  uint64_t iteration = 0;
  while (1) {
    auto* cpu_sched = per_cpu::GetCurrentCore().sched_data;
    auto start_tick = cpu_sched->local_tick;
    klog::Info("Task4: sleeping for 4s (iteration %llu)\n", iteration++);
    sys_sleep(4000);
    auto end_tick = cpu_sched->local_tick;
    klog::Info("Task4: woke up (slept ~%llu ticks)\n", end_tick - start_tick);
  }
}

/// 为当前核心创建测试任务
void create_test_tasks() {
  size_t core_id = cpu_io::GetCurrentCoreId();
  auto& tm = Singleton<TaskManager>::GetInstance();

  auto task1 = new TaskControlBlock("Task1-Exit", 10, task1_func,
                                    reinterpret_cast<void*>(0x1111));
  auto task2 = new TaskControlBlock("Task2-Yield", 10, task2_func,
                                    reinterpret_cast<void*>(0x2222));
  auto task3 = new TaskControlBlock("Task3-Sync", 10, task3_func,
                                    reinterpret_cast<void*>(0x3333));
  auto task4 = new TaskControlBlock("Task4-Sleep", 10, task4_func,
                                    reinterpret_cast<void*>(0x4444));

  // 设置 CPU 亲和性，绑定到当前核心
  task1->cpu_affinity = (1UL << core_id);
  task2->cpu_affinity = (1UL << core_id);
  task3->cpu_affinity = (1UL << core_id);
  task4->cpu_affinity = (1UL << core_id);

  tm.AddTask(task1);
  tm.AddTask(task2);
  tm.AddTask(task3);
  tm.AddTask(task4);

  klog::Info("Created 4 test tasks\n");
}

/// 非启动核入口
auto main_smp(int argc, const char** argv) -> int {
  per_cpu::GetCurrentCore() = per_cpu::PerCpu(cpu_io::GetCurrentCoreId());
  ArchInitSMP(argc, argv);
  MemoryInitSMP();
  InterruptInitSMP(argc, argv);
  Singleton<TaskManager>::GetInstance().InitCurrentCore();

  klog::Info("Hello SimpleKernel SMP\n");

  // 为当前核心创建测试任务
  create_test_tasks();

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

  while (true) {
    cpu_io::Pause();
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
  klog::Info("Initializing test tasks...\n");

  // 为主核心创建测试任务
  create_test_tasks();

  klog::Info("Main: Starting scheduler...\n");

  // 启动调度器，不再返回
  // 从这里开始，系统将在各个任务之间切换
  Singleton<TaskManager>::GetInstance().Schedule();

  // 不应该执行到这里
  __builtin_unreachable();
}
