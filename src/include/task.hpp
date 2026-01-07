/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_TASK_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_TASK_HPP_

#include <cpu_io.h>

#include <array>
#include <cstddef>
#include <cstdint>

/**
 * @brief 任务状态枚举
 */
enum TaskStatus : uint8_t {
  // 未初始化
  kUnInit,
  // 以此状态准备运行
  kReady,
  // 正在运行
  kRunning,
  // 已退出
  kExited,
  // 僵尸状态 (等待回收)
  kZombie,
};

/**
 * @brief 任务控制块 (Task Control Block, TCB)
 * 管理进程/线程的核心数据结构
 */
struct TaskControlBlock {
  // 默认内核栈大小 (16 KB)
  static constexpr const size_t kDefaultKernelStackSize = 16 * 1024;

  // 任务名称
  const char* name = "Unnamed Task";
  // 进程 ID
  size_t pid;

  // 进程状态
  TaskStatus status;

  // 内核栈
  std::array<uint8_t, kDefaultKernelStackSize> kernel_stack_top;

  // 当前的 Trap 上下文指针
  // 指向最近一次 Trap 保存的上下文结构体在内核栈上的位置。
  // 调度器通过此指针来恢复任务的上下文。
  cpu_io::TrapContext* trap_context_ptr;

  // 任务上下文 (用于内核线程切换)
  // 如果支持内核级线程切换 (switch)，这里还需要保存 Callee-Saved 寄存器
  cpu_io::CalleeSavedContext task_context;

  // 页表指针
  uint64_t* page_table;

  // CPU 亲和性 (CPU Affinity) Mask
  // 位掩码，每一位代表一个 CPU 核心。如果某一位为 1，表示允许在该 CPU 上运行。
  // Bit 0 -> Core 0, Bit 1 -> Core 1 ...
  // 例如: 0x1 (只在 Core 0), 0x3 (Core 0 或 1), UINT64_MAX (任意 Core)
  uint64_t cpu_affinity;

  // 父进程 ID
  // 0 通常表示初始进程或无父进程
  size_t parent_pid;

  // 其他预留字段:
  // std::vector<std::shared_ptr<File>> fd_table;

  /**
   * @brief 构造函数
   * @param pid 进程 ID
   */
  TaskControlBlock(const char* name, size_t pid);

  /// @name 构造/析构函数
  /// @{
  TaskControlBlock() = default;
  TaskControlBlock(const TaskControlBlock&) = default;
  TaskControlBlock(TaskControlBlock&&) = default;
  auto operator=(const TaskControlBlock&) -> TaskControlBlock& = default;
  auto operator=(TaskControlBlock&&) -> TaskControlBlock& = default;
  ~TaskControlBlock() = default;
  /// @}
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TASK_HPP_
