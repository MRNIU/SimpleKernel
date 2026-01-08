/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task.hpp"

#include <cpu_io.h>

#include <memory>

#include "kernel_log.hpp"
#include "singleton.hpp"

TaskControlBlock::TaskControlBlock()
    : name("Idle"),
      pid(0),
      status(TaskStatus::kRunning),
      page_table(nullptr),
      cpu_affinity(UINT64_MAX),
      parent_pid(0) {
  // 主线程 (Idle) 不需要分配栈，因为它使用当前引导时的栈
  // 但我们必须初始化 trap_context_ptr 防止空指针，虽然它可能不会被直接用到
}

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

// 实现一个简易的 yield
void sys_yield() { TaskManager::GetInstance().Schedule(); }

void TaskControlBlock::InitThread(void (*entry)(void*), void* arg) {
  // 设置内核栈顶
  uint64_t stack_top = reinterpret_cast<uint64_t>(kernel_stack_top.data()) +
                       kernel_stack_top.size();

  // 1. 设置 ra 指向汇编跳板 kernel_thread_entry
  // 当 switch_to 执行 ret 时，会跳转到 kernel_thread_entry
  task_context.ra = reinterpret_cast<uint64_t>(kernel_thread_entry);

  // 2. 设置 s0 保存真正的入口函数地址
  // kernel_thread_entry 会执行 jalr s0
  task_context.s0 = reinterpret_cast<uint64_t>(entry);

  // 3. 设置 s1 保存参数
  // kernel_thread_entry 会执行 mv a0, s1
  task_context.s1 = reinterpret_cast<uint64_t>(arg);

  // 4. 设置 sp 为栈顶
  task_context.sp = stack_top;

  // 状态设为就绪
  status = TaskStatus::kReady;
}

extern "C" void trap_return(void*);

void TaskControlBlock::InitUserThread(void* entry, void* arg, void* user_sp) {
  // 设置内核栈顶
  uint64_t stack_top = reinterpret_cast<uint64_t>(kernel_stack_top.data()) +
                       kernel_stack_top.size();

  // 1. 预留 TrapContext 空间
  trap_context_ptr = reinterpret_cast<cpu_io::TrapContext*>(
      stack_top - sizeof(cpu_io::TrapContext));

  // 2. 初始化 TrapContext (用户上下文)
  *trap_context_ptr = cpu_io::TrapContext();

  // sstatus: SPP=0 (User), SPIE=1 (Enable Interrupts), FS=1 (Initial)
  trap_context_ptr->sstatus = 0x00002020;
  trap_context_ptr->sepc = reinterpret_cast<uint64_t>(entry);
  trap_context_ptr->sp = reinterpret_cast<uint64_t>(user_sp);
  trap_context_ptr->a0 = reinterpret_cast<uint64_t>(arg);

  // 3. 初始化 TaskContext (内核上下文)
  // ra -> kernel_thread_entry
  // s0 -> trap_return
  // s1 -> trap_context_ptr
  task_context.ra = reinterpret_cast<uint64_t>(kernel_thread_entry);
  task_context.s0 = reinterpret_cast<uint64_t>(trap_return);
  task_context.s1 = reinterpret_cast<uint64_t>(trap_context_ptr);
  task_context.sp = reinterpret_cast<uint64_t>(trap_context_ptr);

  // 状态设为就绪
  status = TaskStatus::kReady;
}
