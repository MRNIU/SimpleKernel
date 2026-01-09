/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_TASK_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_TASK_HPP_

#include <cpu_io.h>

#include <array>
#include <cstddef>
#include <cstdint>

#include "interrupt_base.h"
#include "sk_list"

class SchedulerBase;

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
 * @brief 调度策略
 */
enum SchedPolicy : uint8_t {
  kRealTime = 0,  // 实时任务 (最高优先级)
  kNormal = 1,    // 普通任务
  kIdle = 2,      // 空闲任务 (最低优先级)
  kPolicyCount    // 策略数量
};

using ThreadEntry = void (*)(void*);

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
  // 调度策略
  SchedPolicy policy;
  // 优先级 (数字越小优先级越高，用于同策略内部比较，可选)
  int priority;

  // 内核栈
  std::array<uint8_t, kDefaultKernelStackSize> kernel_stack_top;

  // 当前的 Trap 上下文指针
  // 指向最近一次 Trap 保存的上下文结构体在内核栈上的位置。
  // 调度器通过此指针来恢复任务的上下文。
  cpu_io::TrapContext* trap_context_ptr;

  // 任务上下文 (用于内核线程切换)
  // 保存切换时的内核栈指针 (包含 Callee-Saved 寄存器)
  // 指向各个任务内核栈上保存的 CalleeSavedContext
  cpu_io::CalleeSavedContext task_context;

  // 页表指针
  uint64_t* page_table;

  // CPU 亲和性 (CPU Affinity) Mask
  // 位掩码，每一位代表一个 CPU 核心。如果某一位为 1，表示允许在该 CPU 上运行。
  // Bit 0 -> Core 0, Bit 1 -> Core 1 ...
  // 例如: 0x1 (只在 Core 0), 0x3 (Core 0 或 1), UINT64_MAX (任意 Core)
  uint64_t cpu_affinity;

  // 父进程 ID
  size_t parent_pid;

  // 其他预留字段:
  // std::vector<std::shared_ptr<File>> fd_table;

  /**
   * @brief 构造函数 (内核线程)
   * @param name 任务名称
   * @param pid 进程 ID
   * @param entry 线程入口函数
   * @param arg 线程参数
   */
  TaskControlBlock(const char* name, size_t pid, ThreadEntry entry, void* arg);

  /**
   * @brief 构造函数 (用户线程)
   * @param name 任务名称
   * @param pid 进程 ID
   * @param entry 用户程序入口地址
   * @param arg 用户程序参数
   * @param user_sp 用户栈顶地址
   */
  TaskControlBlock(const char* name, size_t pid, ThreadEntry entry, void* arg,
                   void* user_sp);

  /// @name 构造/析构函数
  /// @{
  TaskControlBlock() = default;
  TaskControlBlock(const TaskControlBlock&) = default;
  TaskControlBlock(TaskControlBlock&&) = default;
  auto operator=(const TaskControlBlock&) -> TaskControlBlock& = default;
  auto operator=(TaskControlBlock&&) -> TaskControlBlock& = default;
  ~TaskControlBlock() = default;
  /// @}

 private:
  /**
   * @brief 初始化内核线程上下文
   * @param entry 线程入口函数
   * @param arg 线程参数
   */
  void InitKernelThread(ThreadEntry entry, void* arg);

  /**
   * @brief 初始化用户线程上下文
   * @param entry 用户程序入口地址
   * @param arg 用户程序参数
   * @param user_sp 用户栈顶地址
   */
  void InitUserThread(ThreadEntry entry, void* arg, void* user_sp);
};

class TaskManager {
 public:
  /// @name 构造/析构函数
  /// @{
  TaskManager();
  TaskManager(const TaskManager&) = default;
  TaskManager(TaskManager&&) = default;
  auto operator=(const TaskManager&) -> TaskManager& = default;
  auto operator=(TaskManager&&) -> TaskManager& = default;
  ~TaskManager() = default;
  /// @}

  // 添加任务：根据策略分发到对应的调度器
  void AddTask(TaskControlBlock* task);

  void Schedule();

  TaskControlBlock* GetCurrentTask() const { return current_task; }

  // 初始化主线程
  void InitMainThread();

 private:
  // 调度器数组，索引对应 SchedPolicy
  SchedulerBase* schedulers[SchedPolicy::kPolicyCount];

  TaskControlBlock* current_task = nullptr;
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TASK_HPP_
