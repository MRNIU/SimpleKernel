/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "syscall.hpp"

#include "kernel.h"
#include "kernel_log.hpp"
#include "task_manager.hpp"

int syscall_dispatcher(int64_t syscall_id, uint64_t args[6]) {
  int64_t ret = 0;
  switch (syscall_id) {
    case SYSCALL_WRITE:
      ret = sys_write(static_cast<int>(args[0]),
                      reinterpret_cast<const char*>(args[1]),
                      static_cast<size_t>(args[2]));
      break;
    case SYSCALL_EXIT:
      ret = sys_exit(static_cast<int>(args[0]));
      break;
    case SYSCALL_YIELD:
      ret = sys_yield();
      break;
    case SYSCALL_CLONE:
      ret = sys_clone(args[0], reinterpret_cast<void*>(args[1]),
                      reinterpret_cast<int*>(args[2]),
                      reinterpret_cast<int*>(args[3]),
                      reinterpret_cast<void*>(args[4]));
      break;
    case SYSCALL_FORK:
      ret = sys_fork();
      break;
    case SYSCALL_GETTID:
      ret = sys_gettid();
      break;
    case SYSCALL_SET_TID_ADDRESS:
      ret = sys_set_tid_address(reinterpret_cast<int*>(args[0]));
      break;
    case SYSCALL_FUTEX:
      ret = sys_futex(
          reinterpret_cast<int*>(args[0]), static_cast<int>(args[1]),
          static_cast<int>(args[2]), reinterpret_cast<const void*>(args[3]),
          reinterpret_cast<int*>(args[4]), static_cast<int>(args[5]));
      break;
    default:
      klog::Err("[Syscall] Unknown syscall id: {}\n", syscall_id);
      ret = -1;
      break;
  }
  return ret;
}

int sys_write(int fd, const char* buf, size_t len) {
  // 简单实现：仅支持向标准输出(1)和错误输出(2)打印
  if (fd == 1 || fd == 2) {
    /// @todo应该检查 buf 是否在用户空间合法范围内
    for (size_t i = 0; i < len; ++i) {
      // 使用 klog::Print 而不是 Format，避免 buffer 解析带来的问题
      sk_putchar(buf[i], nullptr);
    }
    return static_cast<int>(len);
  }
  return -1;
}

int sys_exit(int code) {
  klog::Info("[Syscall] Process {} exited with code {}\n",
             TaskManagerSingleton::instance().GetCurrentTask()->pid, code);
  // 调用 TaskManager 的 Exit 方法处理线程退出
  TaskManagerSingleton::instance().Exit(code);
  // 不会执行到这里，因为 Exit 会触发调度切换
  klog::Err("[Syscall] sys_exit should not return!\n");
  return 0;
}

int sys_yield() {
  TaskManagerSingleton::instance().Schedule();
  return 0;
}

int sys_sleep(uint64_t ms) {
  TaskManagerSingleton::instance().Sleep(ms);
  return 0;
}

int sys_clone(uint64_t flags, void* stack, int* parent_tid, int* child_tid,
              void* tls) {
  auto& task_manager = TaskManagerSingleton::instance();
  auto current = task_manager.GetCurrentTask();

  if (!current || !current->trap_context_ptr) {
    klog::Err("[Syscall] sys_clone: Invalid current task or trap context\n");
    return -1;
  }

  // 调用 TaskManager 的 Clone 方法
  auto result = task_manager.Clone(flags, stack, parent_tid, child_tid, tls,
                                   *current->trap_context_ptr);

  if (!result.has_value()) {
    // 失败返回 -1
    klog::Err("[Syscall] sys_clone failed: {}\n", result.error().message());
    return -1;
  }

  Pid child_pid = result.value();
  if (child_pid == 0) {
    // 子进程/线程返回 0
    return 0;
  } else {
    // 父进程返回子进程 PID
    return static_cast<int>(child_pid);
  }
}

int sys_fork() {
  auto& task_manager = TaskManagerSingleton::instance();
  auto current = task_manager.GetCurrentTask();

  if (!current || !current->trap_context_ptr) {
    klog::Err("[Syscall] sys_fork: Invalid current task or trap context\n");
    return -1;
  }

  // fork = clone with flags=0 (完全复制，不共享任何资源)
  auto result = task_manager.Clone(0, nullptr, nullptr, nullptr, nullptr,
                                   *current->trap_context_ptr);

  if (!result.has_value()) {
    // 失败返回 -1
    klog::Err("[Syscall] sys_fork failed: {}\n", result.error().message());
    return -1;
  }

  Pid child_pid = result.value();
  if (child_pid == 0) {
    // 子进程返回 0
    return 0;
  } else {
    // 父进程返回子进程 PID
    return static_cast<int>(child_pid);
  }
}

int sys_gettid() {
  auto current = TaskManagerSingleton::instance().GetCurrentTask();
  if (!current) {
    klog::Err("[Syscall] sys_gettid: No current task\n");
    return -1;
  }
  return static_cast<int>(current->pid);
}

int sys_set_tid_address([[maybe_unused]] int* tidptr) {
  auto current = TaskManagerSingleton::instance().GetCurrentTask();
  if (!current) {
    klog::Err("[Syscall] sys_set_tid_address: No current task\n");
    return -1;
  }

  // 保存 tidptr，在线程退出时会清零该地址并执行 futex wake
  /// @todo需要在 TaskControlBlock 中添加字段保存 tidptr
  // current->clear_child_tid = tidptr;

  // 返回当前线程的 TID
  return static_cast<int>(current->pid);
}

int sys_futex(int* uaddr, int op, int val, [[maybe_unused]] const void* timeout,
              [[maybe_unused]] int* uaddr2, [[maybe_unused]] int val3) {
  // Futex 常量定义
  constexpr int FUTEX_WAIT = 0;
  constexpr int FUTEX_WAKE = 1;
  constexpr int FUTEX_REQUEUE = 3;

  // 提取操作类型（低位）
  int cmd = op & 0x7F;

  auto& task_manager = TaskManagerSingleton::instance();

  switch (cmd) {
    case FUTEX_WAIT: {
      // 检查 *uaddr 是否等于 val，如果相等则阻塞
      /// @todo需要实现原子比较和阻塞逻辑
      /// @todo需要在 TaskManager 中添加 futex 等待队列
      klog::Debug("[Syscall] FUTEX_WAIT on {:#x} (val={})\n",
                  reinterpret_cast<uintptr_t>(uaddr), val);

      // 简化实现：直接检查值并阻塞
      if (*uaddr == val) {
        // 使用 Futex 类型的资源 ID，地址作为标识
        ResourceId futex_id(ResourceType::kFutex,
                            reinterpret_cast<uintptr_t>(uaddr));
        task_manager.Block(futex_id);
      }
      return 0;
    }

    case FUTEX_WAKE: {
      // 唤醒最多 val 个等待 uaddr 的线程
      klog::Debug("[Syscall] FUTEX_WAKE on {:#x} (count={})\n",
                  reinterpret_cast<uintptr_t>(uaddr), val);

      // 唤醒等待该 futex 的所有线程（简化实现，应该只唤醒 val 个）
      ResourceId futex_id(ResourceType::kFutex,
                          reinterpret_cast<uintptr_t>(uaddr));
      task_manager.Wakeup(futex_id);

      /// @todo应该返回实际唤醒的线程数
      return val;
    }

    case FUTEX_REQUEUE: {
      // 将等待 uaddr 的线程重新排队到 uaddr2
      /// @todo实现 FUTEX_REQUEUE
      klog::Warn("[Syscall] FUTEX_REQUEUE not implemented\n");
      return -1;
    }

    default:
      klog::Err("[Syscall] Unknown futex operation: {}\n", cmd);
      return -1;
  }
}

int sys_sched_getaffinity(int pid, size_t cpusetsize, uint64_t* mask) {
  auto& task_manager = TaskManagerSingleton::instance();

  TaskControlBlock* target;
  if (pid == 0) {
    // pid=0 表示当前线程
    target = task_manager.GetCurrentTask();
  } else {
    // 查找指定 PID 的任务
    target = task_manager.FindTask(static_cast<Pid>(pid));
    if (!target) {
      klog::Err("[Syscall] sys_sched_getaffinity: Task {} not found\n", pid);
      return -1;
    }
  }

  if (!target) {
    return -1;
  }

  // 简单实现：将 cpu_affinity 复制到用户空间
  if (cpusetsize < sizeof(uint64_t)) {
    return -1;
  }

  *mask = target->cpu_affinity;
  return 0;
}

int sys_sched_setaffinity(int pid, size_t cpusetsize, const uint64_t* mask) {
  auto& task_manager = TaskManagerSingleton::instance();

  TaskControlBlock* target;
  if (pid == 0) {
    target = task_manager.GetCurrentTask();
  } else {
    // 查找指定 PID 的任务
    target = task_manager.FindTask(static_cast<Pid>(pid));
    if (!target) {
      klog::Err("[Syscall] sys_sched_setaffinity: Task {} not found\n", pid);
      return -1;
    }
  }

  if (!target) {
    return -1;
  }

  // 简单实现：更新 cpu_affinity
  if (cpusetsize < sizeof(uint64_t)) {
    return -1;
  }

  target->cpu_affinity = *mask;

  klog::Debug("[Syscall] Set CPU affinity for task {} to {:#x}\n", target->pid,
              *mask);

  /// @todo 如果当前任务不在允许的 CPU 上运行，应该触发迁移

  return 0;
}
