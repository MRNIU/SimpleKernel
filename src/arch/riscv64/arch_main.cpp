/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>
#include <opensbi_interface.h>

#include <cstring>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
#include "sk_cstdio"
#include "sk_stdlib.h"
#include "virtual_memory.hpp"

BasicInfo::BasicInfo(int, const char** argv) {
  Singleton<KernelFdt>::GetInstance()
      .GetMemory()
      .and_then([this](std::pair<uint64_t, size_t> mem) -> Expected<void> {
        physical_memory_addr = mem.first;
        physical_memory_size = mem.second;
        return {};
      })
      .or_else([](Error err) -> Expected<void> {
        klog::Err("Failed to get memory info: %s\n", err.message());
        while (true) {
          cpu_io::Pause();
        }
        return {};
      });

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);
  elf_addr = kernel_addr;

  fdt_addr = reinterpret_cast<uint64_t>(argv);

  core_count = Singleton<KernelFdt>::GetInstance().GetCoreCount().value_or(1);

  interval =
      Singleton<KernelFdt>::GetInstance().GetTimebaseFrequency().value_or(0);
}

void ArchInit(int argc, const char** argv) {
  Singleton<KernelFdt>::GetInstance() =
      KernelFdt(reinterpret_cast<uint64_t>(argv));

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr);

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

void InitTaskContext(cpu_io::CalleeSavedContext* task_context,
                     void (*entry)(void*), void* arg, uint64_t stack_top) {
  // 清零上下文
  std::memset(task_context, 0, sizeof(cpu_io::CalleeSavedContext));

  // 1. 设置 ra 指向汇编跳板 kernel_thread_entry
  // 当 switch_to 执行 ret 时，会跳转到 kernel_thread_entry
  task_context->ra = reinterpret_cast<uint64_t>(kernel_thread_entry);

  // 2. 设置 s0 保存真正的入口函数地址
  task_context->s0 = reinterpret_cast<uint64_t>(entry);

  // 3. 设置 s1 保存参数
  task_context->s1 = reinterpret_cast<uint64_t>(arg);

  // 4. 设置 sp 为栈顶
  task_context->sp = stack_top;
}

void InitTaskContext(cpu_io::CalleeSavedContext* task_context,
                     cpu_io::TrapContext* trap_context_ptr,
                     uint64_t stack_top) {
  // 清零上下文
  std::memset(task_context, 0, sizeof(cpu_io::CalleeSavedContext));

  // 设置 ra 指向 kernel_thread_entry
  task_context->ra = reinterpret_cast<uint64_t>(kernel_thread_entry);

  // 设置 s0 指向 trap_return，用于返回用户态
  task_context->s0 = reinterpret_cast<uint64_t>(trap_return);

  // 设置 s1 指向 trap_context_ptr
  task_context->s1 = reinterpret_cast<uint64_t>(trap_context_ptr);

  // 设置 sp 为栈顶
  task_context->sp = stack_top;
}
