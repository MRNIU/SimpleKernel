/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstring>

#include "cpu_io.h"

extern "C" {

void switch_to(cpu_io::CalleeSavedContext*, cpu_io::CalleeSavedContext*) {}
void kernel_thread_entry() {}
void trap_return(void*) {}
void trap_entry() {}

}  // extern "C"

void InitTaskContext(cpu_io::CalleeSavedContext* task_context,
                     void (*entry)(void*), void* arg, uint64_t stack_top) {
  // 清零上下文
  std::memset(task_context, 0, sizeof(cpu_io::CalleeSavedContext));

  task_context->ReturnAddress() =
      reinterpret_cast<uint64_t>(kernel_thread_entry);
  task_context->EntryFunction() = reinterpret_cast<uint64_t>(entry);
  task_context->EntryArgument() = reinterpret_cast<uint64_t>(arg);
  task_context->StackPointer() = stack_top;
}

void InitTaskContext(cpu_io::CalleeSavedContext* task_context,
                     cpu_io::TrapContext* trap_context_ptr,
                     uint64_t stack_top) {
  // 清零上下文
  std::memset(task_context, 0, sizeof(cpu_io::CalleeSavedContext));

  task_context->ReturnAddress() =
      reinterpret_cast<uint64_t>(kernel_thread_entry);
  task_context->EntryFunction() = reinterpret_cast<uint64_t>(trap_return);
  task_context->EntryArgument() = reinterpret_cast<uint64_t>(trap_context_ptr);
  task_context->StackPointer() = stack_top;
}
