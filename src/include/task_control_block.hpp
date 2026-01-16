/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_TASK_CONTROL_BLOCK_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_TASK_CONTROL_BLOCK_HPP_

#include <cpu_io.h>

#include <array>
#include <cstddef>
#include <cstdint>

/// 进程 ID 类型
using Pid = size_t;

/// 线程入口函数类型
using ThreadEntry = void (*)(void*);

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
  // 睡眠中
  kSleeping,
  // 已退出
  kExited,
  // 僵尸状态 (等待回收)
  kZombie,
};

/**
 * @brief 调度策略
 */
enum SchedPolicy : uint8_t {
  // 实时任务 (最高优先级)
  kRealTime = 0,
  // 普通任务
  kNormal = 1,
  // 空闲任务 (最低优先级)
  kIdle = 2,
  // 策略数量
  kPolicyCount
};

/**
 * @brief 任务控制块，管理进程/线程的核心数据结构
 */
struct TaskControlBlock {
  /// 默认内核栈大小 (16 KB)
  static constexpr const size_t kDefaultKernelStackSize = 16 * 1024;

  struct PriorityCompare {
    bool operator()(TaskControlBlock* a, TaskControlBlock* b) {
      // 优先级数值越小，优先级越高
      return a->priority > b->priority;
    }
  };

  struct WakeTickCompare {
    bool operator()(TaskControlBlock* a, TaskControlBlock* b) {
      return a->wake_tick > b->wake_tick;
    }
  };

  /// 任务名称
  const char* name = "Unnamed Task";

  /// 进程 ID
  Pid pid;

  /// 进程状态
  TaskStatus status = TaskStatus::kUnInit;

  // 调度策略
  SchedPolicy policy = SchedPolicy::kNormal;

  /// 优先级 (数字越小优先级越高，用于同策略内部比较，可选)
  int priority = 10;

  /// 唤醒时间 (tick)
  uint64_t wake_tick = 0;

  /// 剩余时间片 (单位: ticks)
  uint64_t time_slice_remaining = 10;

  /// 默认时间片 (单位: ticks)
  uint64_t time_slice_default = 10;

  /// 总运行时间 (单位: ticks，用于统计)
  uint64_t total_runtime = 0;

  /// 上下文切换次数 (用于统计)
  uint64_t context_switches = 0;

  /// 内核栈
  std::array<uint8_t, kDefaultKernelStackSize> kernel_stack_top{};

  /**
   * @brief 当前的 Trap 上下文指针
   *
   * 指向最近一次 Trap 保存的上下文结构体在内核栈上的位置。
   * 调度器通过此指针来恢复任务的上下文。
   */
  cpu_io::TrapContext* trap_context_ptr = nullptr;

  /**
   * @brief 任务上下文 (用于内核线程切换)
   *
   * 保存切换时的内核栈指针 (包含 Callee-Saved 寄存器)。
   * 指向各个任务内核栈上保存的 CalleeSavedContext。
   */
  cpu_io::CalleeSavedContext task_context{};

  /// 页表指针
  uint64_t* page_table = nullptr;

  /**
   * @brief CPU 亲和性 (CPU Affinity) 位掩码
   */
  uint64_t cpu_affinity = UINT64_MAX;

  /// 父进程 ID
  Pid parent_pid = 0;

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
   * @param elf 指向 ELF 镜像的指针
   * @param argc 参数个数
   * @param argv 参数数组
   */
  TaskControlBlock(const char* name, size_t pid, uint8_t* elf, int argc,
                   char** argv);

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

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TASK_CONTROL_BLOCK_HPP_
