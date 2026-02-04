/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstring>

#include "cpu_io.h"
#include "test_environment_state.hpp"

extern "C" {

void switch_to(cpu_io::CalleeSavedContext* prev_ctx,
               cpu_io::CalleeSavedContext* next_ctx) {
  auto& env_state = test_env::TestEnvironmentState::GetInstance();
  auto& core = env_state.GetCurrentCoreEnv();

  // 从上下文指针查找对应的任务
  auto* prev_task = env_state.FindTaskByContext(prev_ctx);
  auto* next_task = env_state.FindTaskByContext(next_ctx);

  // 记录切换事件
  core.switch_history.push_back({
      .timestamp = core.local_tick,
      .from = prev_task,
      .to = next_task,
      .core_id = core.core_id,
  });

  // 更新当前线程
  core.current_thread = next_task;
  core.total_switches++;

  // 在单元测试中不执行真实的栈切换
}
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
