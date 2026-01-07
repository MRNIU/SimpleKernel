/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task.hpp"

#include <cpu_io.h>

#include <cstring>

TaskControlBlock::TaskControlBlock(const char* name, size_t pid)
    : name(name),
      pid(pid),
      status(TaskStatus::kUnInit),
      page_table(nullptr),
      cpu_affinity(UINT64_MAX),
      parent_pid(0) {
  // 初始化 TrapContext 指针的位置 (预留空间)
  // 实际上只有在 trap 发生时才会真正写入，但我们可以预先设置好位置
  trap_context_ptr = reinterpret_cast<cpu_io::TrapContext*>(
      kernel_stack_top.data() + kernel_stack_top.size() -
      sizeof(cpu_io::TrapContext));
}
