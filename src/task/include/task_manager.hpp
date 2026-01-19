/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_TASK_MANAGER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_TASK_MANAGER_HPP_

#include <cpu_io.h>

#include <array>
#include <cstddef>
#include <cstdint>

#include "interrupt_base.h"
#include "per_cpu.hpp"
#include "scheduler_base.hpp"
#include "sk_list"
#include "sk_priority_queue"
#include "spinlock.hpp"
#include "task_control_block.hpp"

/**
 * @brief 每个核心的调度数据 (RunQueue)
 */
struct CpuSchedData {
  SpinLock lock{"sched_lock"};
  std::array<SchedulerBase*, SchedPolicy::kPolicyCount> schedulers{};
  sk_std::priority_queue<TaskControlBlock*, sk_std::vector<TaskControlBlock*>,
                         TaskControlBlock::WakeTickCompare>
      sleeping_tasks;

  /// Per-CPU tick 计数 (每个核心独立计时)
  uint64_t local_tick = 0;

  /// 本核心的空闲时间 (单位: ticks)
  uint64_t idle_time = 0;

  /// 本核心的总调度次数
  uint64_t total_schedules = 0;
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
  TaskManager() = default;
  TaskManager(const TaskManager&) = delete;
  TaskManager(TaskManager&&) = delete;
  auto operator=(const TaskManager&) -> TaskManager& = delete;
  auto operator=(TaskManager&&) -> TaskManager& = delete;
  ~TaskManager() = default;
  /// @}

  /**
   * @brief 为当前核心创建主线程任务
   *
   * 将当前正在执行的流包装为主线程任务。
   */
  void InitCurrentCore();

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
   * @brief 更新系统 tick
   */
  void TickUpdate();

  /**
   * @brief 线程睡眠
   * @param ms 睡眠毫秒数
   */
  void Sleep(uint64_t ms);

  /**
   * @brief 退出当前线程
   * @param exit_code 退出码
   */
  void Exit(int exit_code = 0);

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
  std::array<CpuSchedData, SIMPLEKERNEL_MAX_CORE_COUNT> cpu_schedulers_;

  /**
   * @brief 获取当前核心的调度数据
   */
  CpuSchedData& GetCurrentCpuSched() {
    return cpu_schedulers_[cpu_io::GetCurrentCoreId()];
  }

  /**
   * @brief 系统启动时间 (单位: ticks，用于全局时间参考)
   */
  std::atomic<uint64_t> system_boot_tick{0};

  /**
   * @brief PID 分配器
   */
  std::atomic<size_t> pid_allocator{1};
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TASK_MANAGER_HPP_
