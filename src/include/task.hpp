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
  // 0 通常表示初始进程或无父进程
  size_t parent_pid;

  // 其他预留字段:
  // std::vector<std::shared_ptr<File>> fd_table;

  /**
   * @brief 构造函数
   * @param pid 进程 ID
   */
  TaskControlBlock(const char* name, size_t pid);

  /**
   * @brief 初始化内核线程上下文
   * @param entry 线程入口函数
   * @param arg 线程参数
   */
  void InitThread(void (*entry)(void*), void* arg);

  /// @name 构造/析构函数
  /// @{
  TaskControlBlock();
  TaskControlBlock(const TaskControlBlock&) = default;
  TaskControlBlock(TaskControlBlock&&) = default;
  auto operator=(const TaskControlBlock&) -> TaskControlBlock& = default;
  auto operator=(TaskControlBlock&&) -> TaskControlBlock& = default;
  ~TaskControlBlock() = default;
  /// @}
};

// 供汇编调用：上下文切换
extern "C" void switch_to(cpu_io::CalleeSavedContext* prev,
                          cpu_io::CalleeSavedContext* next);

// 汇编入口跳板
extern "C" void kernel_thread_entry();

class TaskManager {
 public:
  static TaskManager& GetInstance() {
    static TaskManager instance;
    return instance;
  }

  void AddTask(TaskControlBlock* task) {
    if (ready_count < kMaxReadyTasks) {
      ready_queue[ready_count++] = task;
    }
  }

  void Schedule() {
    if (ready_count == 0) {
      return;
    }

    // 简单的 FIFO 调度
    // 这是一个低效的数组移动实现，仅作为临时展示
    TaskControlBlock* next_task = ready_queue[0];

    // 移动数组元素
    for (size_t i = 0; i < ready_count - 1; ++i) {
      ready_queue[i] = ready_queue[i + 1];
    }
    ready_count--;

    // 如果当前任务还在运行（没有退出），则放回队列末尾
    // 注意：这里我们简单假设 Schedule 是由当前任务主动调用的 (Yield)
    if (current_task->status == TaskStatus::kRunning) {
      current_task->status = TaskStatus::kReady;
      if (ready_count < kMaxReadyTasks) {
        ready_queue[ready_count++] = current_task;
      }
    }

    TaskControlBlock* prev_task = current_task;
    current_task = next_task;
    current_task->status = TaskStatus::kRunning;

    // 执行上下文切换
    switch_to(&prev_task->task_context, &current_task->task_context);
  }

  TaskControlBlock* GetCurrentTask() const { return current_task; }

  // 初始化主线程
  void InitMainThread() {
    static TaskControlBlock main_task;  // Idle/Main thread
    main_task.pid = 0;
    main_task.status = TaskStatus::kRunning;
    current_task = &main_task;
  }

 private:
  TaskManager() = default;

  // 简单的就绪队列 (定长数组 临时替代)
  static constexpr size_t kMaxReadyTasks = 64;
  std::array<TaskControlBlock*, kMaxReadyTasks> ready_queue;
  size_t ready_count = 0;

  TaskControlBlock* current_task = nullptr;
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TASK_HPP_
