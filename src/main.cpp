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
#include "sk_new"
#include "sk_stdlib.h"
#include "syscall.hpp"
#include "task.hpp"
#include "virtual_memory.hpp"

void thread_func_a(void* arg) {
  while (1) {
    klog::Info("Thread A: running, arg=%d\n", (uint64_t)arg);
    sys_sleep(100);
  }
}

void thread_func_b(void* arg) {
  while (1) {
    klog::Info("Thread B: running, arg=%d\n", (uint64_t)arg);
    sys_sleep(100);
  }
}

void user_func(void*) { sys_yield(); }

void user_thread_test() {
  // 1. 获取虚拟内存管理器实例
  auto& vm = Singleton<VirtualMemory>::GetInstance();

  // 2. 分配用户代码页
  void* code_page = aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                  cpu_io::virtual_memory::kPageSize);
  if (!code_page) {
    klog::Err("Failed to allocate user code page\n");
    return;
  }

  // 3. 复制 user_func 到用户代码页
  sk_std::memcpy(code_page, reinterpret_cast<void*>(user_func), 1024);

  // 4. 创建用户页表并映射代码页 (VA = 0x10000000)
  // 分配新的页目录
  void* user_page_dir = aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                      cpu_io::virtual_memory::kPageSize);
  if (!user_page_dir) {
    klog::Err("Failed to allocate user page directory\n");
    return;
  }
  // 复制内核页目录 (共享内核映射)
  void* kernel_page_dir =
      reinterpret_cast<void*>(cpu_io::virtual_memory::GetPageDirectory());
  memcpy(user_page_dir, kernel_page_dir, cpu_io::virtual_memory::kPageSize);

  ThreadEntry user_entry_va = (ThreadEntry)0x10000000;

  vm.MapPage(user_page_dir, reinterpret_cast<void*>(user_entry_va), code_page,
             cpu_io::virtual_memory::kUser | cpu_io::virtual_memory::kRead |
                 cpu_io::virtual_memory::kWrite |
                 cpu_io::virtual_memory::kExec |
                 cpu_io::virtual_memory::kValid);

  // 5. 分配并映射用户栈 (VA = 0x20000000)
  void* stack_page = aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                   cpu_io::virtual_memory::kPageSize);
  uint64_t user_stack_va = 0x20000000;
  vm.MapPage(user_page_dir, reinterpret_cast<void*>(user_stack_va), stack_page,
             cpu_io::virtual_memory::kUser | cpu_io::virtual_memory::kRead |
                 cpu_io::virtual_memory::kWrite |
                 cpu_io::virtual_memory::kValid);

  // 栈顶 (栈向下增长)
  uint64_t user_sp = user_stack_va + cpu_io::virtual_memory::kPageSize;

  // 6. 创建用户线程
  auto user_task =
      new TaskControlBlock("UserDemo", 3, user_entry_va, nullptr,
                           reinterpret_cast<void*>(user_sp), user_page_dir);

  Singleton<TaskManager>::GetInstance().AddTask(user_task);

  klog::Info("User thread created. Entry: 0x%lX\n", user_entry_va);
}

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
  // WakeUpOtherCores();

  DumpStack();

  klog::info << "Hello SimpleKernel\n";

  // 初始化任务管理器 (设置主线程)
  Singleton<TaskManager>::GetInstance().InitMainThread();

  // 运行用户线程测试
  user_thread_test();

  // 创建线程 A
  // 注意：需要手动分配内存，实际中应由 ObjectPool 或 memory allocator
  // 管理生命周期
  auto task_a = new TaskControlBlock("Task A", 1, thread_func_a, (void*)100);
  Singleton<TaskManager>::GetInstance().AddTask(task_a);

  // 创建线程 B
  auto task_b = new TaskControlBlock("Task B", 2, thread_func_b, (void*)200);
  Singleton<TaskManager>::GetInstance().AddTask(task_b);

  klog::Info("Main: Starting scheduler...\n");

  // 主线程进入调度循环
  while (1) {
    klog::Info("Main Thread: running\n");
    sys_sleep(100);
    sys_yield();
  }

  return 0;
}
