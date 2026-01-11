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
#include "per_cpu.hpp"
#include "sk_list"
#include "spinlock.hpp"

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
  TaskStatus status = TaskStatus::kUnInit;
  // 调度策略
  SchedPolicy policy = SchedPolicy::kNormal;
  // 优先级 (数字越小优先级越高，用于同策略内部比较，可选)
  int priority = 10;
  // 唤醒时间 (tick)
  uint64_t wake_tick = 0;
  // 内核栈
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

  // 页表指针
  uint64_t* page_table = nullptr;

  /**
   * @brief CPU 亲和性 (CPU Affinity) Mask
   *
   * 位掩码，每一位代表一个 CPU 核心。如果某一位为 1，表示允许在该 CPU 上运行。
   * Bit 0 -> Core 0, Bit 1 -> Core 1 ...
   * 例如: 0x1 (只在 Core 0), 0x3 (Core 0 或 1), UINT64_MAX (任意 Core)
   */
  uint64_t cpu_affinity = UINT64_MAX;

  // 父进程 ID
  size_t parent_pid = 0;

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

/**
 * @brief 每个核心的调度数据 (RunQueue)
 */
struct CpuSchedData {
  SpinLock lock{"sched_lock"};
  std::array<SchedulerBase*, SchedPolicy::kPolicyCount> schedulers{};
  sk_std::list<TaskControlBlock*> sleeping_tasks;
};

/**
 * @brief 任务管理器
 *
 * 负责管理系统中的所有任务，包括任务的创建、调度、切换等。
 */
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

  /**
   * @brief 添加任务
   *
   * 根据任务的调度策略，将其添加到对应的调度器中。
   *
   * @param task 任务控制块指针
   */
  void AddTask(TaskControlBlock* task);

  /**
   * @brief 调度函数
   *
   * 当前任务时间片耗尽或主动放弃 CPU 时调用。
   * 选择下一个任务并切换上下文。
   */
  void Schedule();

  /**
   * @brief 获取当前任务
   * @return TaskControlBlock* 当前正在运行的任务
   */
  TaskControlBlock* GetCurrentTask() const {
    return per_cpu::GetCurrentCore().running_task;
  }

  /**
   * @brief 为当前核心创建主线程任务
   *
   * 将当前正在执行的流包装为主线程任务。
   */
  void InitCurrentCore();

  /**
   * @brief 更新系统 tick
   */
  void UpdateTick();

  /**
   * @brief 线程睡眠
   * @param ms 睡眠毫秒数
   */
  void Sleep(uint64_t ms);

  /**
   * @brief 设置 tick 频率
   * @param freq 每秒 tick 数
   */
  void SetTickFrequency(uint64_t freq) { tick_frequency = freq; }

  /**
   * @brief 分配新的 PID
   * @return size_t 新的 PID
   */
  size_t AllocatePid();

  /**
   * @brief 负载均衡 (空闲 core 窃取任务)
   */
  void Balance();

 private:
  /**
   * @brief 每个核心的调度数据
   */
  std::array<CpuSchedData, per_cpu::PerCpu::kMaxCoreCount> cpu_schedulers_;

  /**
   * @brief 获取当前核心的调度数据
   */
  CpuSchedData& GetCurrentCpuSched() {
    return cpu_schedulers_[cpu_io::GetCurrentCoreId()];
  }

  /**
   * @brief 系统当前 tick (使用原子变量或每核独立
   * tick，这里为简单起见改为每核独立或者是原子) 但为了兼容
   * sleep，暂时保留为全局 tick (由主核更新) 或者 每个核维护自己的 tick count
   * 如果是多核，每个核都有 timer 中断，所以应该是 Per-Cpu Tick
   */
  // uint64_t current_tick = 0;
  // 移到 CpuSchedData 或者作为 atomic
  std::atomic<uint64_t> current_tick{0};

  /**
   * @brief tick 频率 (Hz)
   */
  uint64_t tick_frequency = 100;

  /**
   * @brief PID 分配器
   */
  std::atomic<size_t> pid_allocator{1};
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TASK_HPP_
