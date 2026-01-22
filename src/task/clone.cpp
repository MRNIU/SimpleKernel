/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kernel_log.hpp"
#include "singleton.hpp"
#include "sk_cstring"
#include "sk_stdlib.h"
#include "task_manager.hpp"

Pid TaskManager::Clone(uint64_t flags, void* user_stack, int* parent_tid,
                       int* child_tid, void* tls,
                       cpu_io::TrapContext& parent_context) {
  auto* parent = GetCurrentTask();
  if (!parent) {
    klog::Err("Clone: No current task\n");
    return -1;
  }

  // 分配新的 PID
  Pid new_pid = AllocatePid();

  // 创建子任务控制块
  auto* child = new TaskControlBlock();
  if (!child) {
    klog::Err("Clone: Failed to allocate child task\n");
    return -1;
  }

  // 基本字段设置
  child->pid = new_pid;
  child->name = parent->name;
  child->status = TaskStatus::kReady;
  child->policy = parent->policy;
  child->sched_info = parent->sched_info;

  // 设置父进程 ID
  if (flags & kCloneParent) {
    // 保持与父进程相同的父进程
    child->parent_pid = parent->parent_pid;
  } else {
    child->parent_pid = parent->pid;
  }

  // 处理线程组 ID (TGID)
  if (flags & kCloneThread) {
    // 创建线程: 共享线程组
    child->tgid = parent->tgid;
    child->pgid = parent->pgid;
    child->sid = parent->sid;

    // 将子线程加入父线程的线程组
    if (parent->IsThreadGroupLeader()) {
      child->JoinThreadGroup(parent);
    } else {
      // 找到线程组的主线程
      TaskControlBlock* leader = FindTask(parent->tgid);
      if (leader) {
        child->JoinThreadGroup(leader);
      } else {
        klog::Warn("Clone: Thread group leader not found for tgid=%zu\n",
                   parent->tgid);
        child->tgid = parent->tgid;
      }
    }
  } else {
    // 创建进程: 新的线程组
    child->tgid = new_pid;  // 新进程的 tgid 是自己的 pid
    child->pgid = parent->pgid;
    child->sid = parent->sid;
  }

  // 克隆标志位
  child->clone_flags = static_cast<CloneFlags>(flags);

  // 处理地址空间
  if (flags & kCloneVm) {
    // 共享地址空间
    child->page_table = parent->page_table;
  } else {
    // 复制地址空间
    /// @todo 实现页表的深拷贝
    child->page_table = nullptr;
  }

  // 分配内核栈
  child->kernel_stack = static_cast<uint8_t*>(
      aligned_alloc(cpu_io::virtual_memory::kPageSize,
                    TaskControlBlock::kDefaultKernelStackSize));
  if (!child->kernel_stack) {
    klog::Err("Clone: Failed to allocate kernel stack\n");
    delete child;
    return -1;
  }

  // 复制 trap context
  child->trap_context_ptr = reinterpret_cast<cpu_io::TrapContext*>(
      child->kernel_stack + TaskControlBlock::kDefaultKernelStackSize -
      sizeof(cpu_io::TrapContext));
  memcpy(child->trap_context_ptr, &parent_context, sizeof(cpu_io::TrapContext));

  // 设置用户栈
  if (user_stack) {
    /// @todo 根据架构设置栈指针寄存器
    // child->trap_context_ptr->sp = (uint64_t)user_stack;
  }

  // 设置 TLS
  if (tls) {
    /// @todo 根据架构设置 TLS 寄存器
  }

  // 设置返回值：父进程返回子进程 PID，子进程返回 0
  /// @todo 根据架构设置返回值寄存器
  // parent_context.a0 = new_pid;  // 父进程返回值
  // child->trap_context_ptr->a0 = 0;  // 子进程返回值

  // 写入 TID
  if (parent_tid) {
    *parent_tid = new_pid;
  }
  if (child_tid) {
    *child_tid = new_pid;
  }

  // 将子任务加入任务表
  {
    LockGuard lock_guard(task_table_lock_);
    task_table_[new_pid] = child;
  }

  // 将子任务添加到调度器
  AddTask(child);

  klog::Debug("Clone: parent=%zu, child=%zu, flags=0x%lx, tgid=%zu\n",
              parent->pid, new_pid, flags, child->tgid);

  return new_pid;
}
