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
#include "sk_unordered_map"
#include "sk_vector"
#include "spinlock.hpp"
#include "task_control_block.hpp"

/**
 * @brief 每个核心的调度数据 (RunQueue)
 */
struct CpuSchedData {
  SpinLock lock{"sched_lock"};

  /// 调度器数组 (按策略索引)
  std::array<SchedulerBase*, SchedPolicy::kPolicyCount> schedulers{};

  /// 睡眠队列 (优先队列，按唤醒时间排序)
  sk_std::priority_queue<TaskControlBlock*, sk_std::vector<TaskControlBlock*>,
                         TaskControlBlock::WakeTickCompare>
      sleeping_tasks;

  /// 阻塞队列 (按资源 ID 分组)
  sk_std::unordered_map<uint64_t, sk_std::list<TaskControlBlock*>>
      blocked_tasks;

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
  /**
   * @brief 初始化 per cpu 的调度数据，创建 idle 线程
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
   * 选择下一个任务并切换上下文
   *
   * @note 被调用意味着需要调度决策，可能是
   * 时间片耗尽（TickUpdate 检测到需要抢占）
   * 主动让出 CPU (yield)
   * 任务阻塞、睡眠或退出
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
  [[noreturn]] void Exit(int exit_code = 0);

  /**
   * @brief 阻塞当前任务
   * @param resource_id 等待的资源 ID (如信号量地址、文件描述符等)
   */
  void Block(uint64_t resource_id);

  /**
   * @brief 唤醒等待指定资源的所有任务
   * @param resource_id 资源 ID
   */
  void Wakeup(uint64_t resource_id);

  /**
   * @brief 分配新的 PID
   * @return size_t 新的 PID
   */
  size_t AllocatePid();

  /**
   * @brief 负载均衡 (空闲 core 窃取任务)
   */
  void Balance();

  /// @name 构造/析构函数
  /// @{
  TaskManager() = default;
  TaskManager(const TaskManager&) = delete;
  TaskManager(TaskManager&&) = delete;
  auto operator=(const TaskManager&) -> TaskManager& = delete;
  auto operator=(TaskManager&&) -> TaskManager& = delete;
  ~TaskManager();
  /// @}

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
   * @brief PID 分配器
   */
  std::atomic<size_t> pid_allocator{1};
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TASK_MANAGER_HPP_
