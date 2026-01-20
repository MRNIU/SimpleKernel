/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstdlib>
#include <cstring>

#include "task_control_block.hpp"

// Mock 实现用于单元测试
// 简化版本，不依赖内核其他组件

TaskControlBlock::TaskControlBlock([[maybe_unused]] const char* name,
                                   [[maybe_unused]] size_t pid,
                                   [[maybe_unused]] ThreadEntry entry,
                                   [[maybe_unused]] void* arg)
    : name(name), pid(pid) {
  // 简化版本：只分配内核栈，不初始化复杂的上下文
  kernel_stack = static_cast<uint8_t*>(aligned_alloc(
      cpu_io::virtual_memory::kPageSize, kDefaultKernelStackSize));

  if (kernel_stack) {
    std::memset(kernel_stack, 0, kDefaultKernelStackSize);
    status = TaskStatus::kReady;
  } else {
    status = TaskStatus::kExited;
  }

  // 简单初始化 trap_context_ptr（指向栈顶预留位置）
  if (kernel_stack) {
    trap_context_ptr = reinterpret_cast<cpu_io::TrapContext*>(
        kernel_stack + kDefaultKernelStackSize - sizeof(cpu_io::TrapContext));
  }
}

TaskControlBlock::TaskControlBlock([[maybe_unused]] const char* name,
                                   [[maybe_unused]] size_t pid,
                                   [[maybe_unused]] uint8_t* elf,
                                   [[maybe_unused]] int argc,
                                   [[maybe_unused]] char** argv)
    : name(name), pid(pid) {
  // 简化版本：只分配内核栈
  kernel_stack = static_cast<uint8_t*>(aligned_alloc(
      cpu_io::virtual_memory::kPageSize, kDefaultKernelStackSize));

  if (kernel_stack) {
    std::memset(kernel_stack, 0, kDefaultKernelStackSize);
    status = TaskStatus::kReady;
  } else {
    status = TaskStatus::kExited;
  }

  // 简单初始化 trap_context_ptr
  if (kernel_stack) {
    trap_context_ptr = reinterpret_cast<cpu_io::TrapContext*>(
        kernel_stack + kDefaultKernelStackSize - sizeof(cpu_io::TrapContext));
  }
}

TaskControlBlock::~TaskControlBlock() {
  // 释放内核栈
  if (kernel_stack) {
    free(kernel_stack);
    kernel_stack = nullptr;
  }

  // 简化版本不需要释放页表
  page_table = nullptr;
}
