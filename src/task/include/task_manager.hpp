/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_TASK_MANAGER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_TASK_MANAGER_HPP_

#include <cpu_io.h>
#include <etl/list.h>
#include <etl/memory.h>
#include <etl/priority_queue.h>
#include <etl/singleton.h>
#include <etl/unordered_map.h>
#include <etl/vector.h>

#include <MPMCQueue.hpp>
#include <array>
#include <cstddef>
#include <cstdint>

#include "expected.hpp"
#include "interrupt_base.h"
#include "kernel_config.hpp"
#include "kstd_memory"
#include "per_cpu.hpp"
#include "resource_id.hpp"
#include "scheduler_base.hpp"
#include "spinlock.hpp"
#include "task_control_block.hpp"

/**
 * @brief 每个核心的调度数据 (RunQueue)
 */
struct CpuSchedData {
  SpinLock lock{"sched_lock"};

  /// 调度器数组 (按策略索引)
  std::array<etl::unique_ptr<SchedulerBase>, SchedPolicy::kPolicyCount>
      schedulers{};

  /// 睡眠队列 (优先队列，按唤醒时间排序)
  etl::priority_queue<
      TaskControlBlock*, kernel::config::kMaxSleepingTasks,
      etl::vector<TaskControlBlock*, kernel::config::kMaxSleepingTasks>,
      TaskControlBlock::WakeTickCompare>
      sleeping_tasks;

  /// 阻塞队列 (按资源 ID 分组)
  etl::unordered_map<
      ResourceId,
      etl::list<TaskControlBlock*, kernel::config::kMaxBlockedPerGroup>,
      kernel::config::kMaxBlockedGroups,
      kernel::config::kMaxBlockedGroupsBuckets>
      blocked_tasks;

  /// Per-CPU tick 计数 (每个核心独立计时)
  uint64_t local_tick = 0;

  /// 本核心的空闲时间 (单位: ticks)
  uint64_t idle_time = 0;

  /// 本核心的总调度次数
  uint64_t total_schedules = 0;

  CpuSchedData() = default;
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
   * @param task 任务控制块指针，状态应为 kUnInit
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
   * @param resource_id 等待的资源 ID
   */
  void Block(ResourceId resource_id);

  /**
   * @brief 唤醒等待指定资源的所有任务
   * @param resource_id 资源 ID
   * @note 会唤醒所有阻塞在此资源上的任务
   */
  void Wakeup(ResourceId resource_id);

  /**
   * @brief 克隆当前任务 (fork/clone 系统调用)
   * @param flags 克隆标志位
   *        - kCloneVm: 共享地址空间
   *        - kCloneThread: 共享线程组
   *        - kCloneFiles: 共享文件描述符表
   *        - kCloneSighand: 共享信号处理器
   *        - 0: 完全复制 (fork)
   * @param user_stack 用户栈指针 (nullptr 表示复制父进程栈)
   * @param parent_tid 父进程 TID 存储地址
   * @param child_tid 子进程 TID 存储地址
   * @param tls 线程局部存储指针
   * @param parent_context 父进程的 trap 上下文 (用于复制寄存器)
   * @return Expected<Pid> 父进程返回子进程 PID，子进程返回 0，失败返回错误
   */
  Expected<Pid> Clone(uint64_t flags, void* user_stack, int* parent_tid,
                      int* child_tid, void* tls,
                      cpu_io::TrapContext& parent_context);

  /**
   * @brief 等待子进程退出
   * @param pid 子进程 PID (-1 表示任意子进程，0 表示同组，>0 表示指定进程)
   * @param status 退出状态存储位置 (可为 nullptr)
   * @param no_hang 非阻塞等待，立即返回 (类似 WNOHANG)
   * @param untraced 报告已停止的子进程 (类似 WUNTRACED)
   * @return Expected<Pid> 成功返回子进程 PID，无子进程或被中断返回错误
   */
  Expected<Pid> Wait(Pid pid, int* status, bool no_hang = false,
                     bool untraced = false);

  /**
   * @brief 按 PID 查找任务
   * @param pid 进程 ID
   * @return TaskControlBlock* 找到的任务，未找到返回 nullptr
   */
  TaskControlBlock* FindTask(Pid pid);

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
  /// 中断工作队列容量
  static constexpr const size_t kInterruptQueueCapacity = 256;

  /// 中断线程处理结构体
  struct InterruptWork {
    using WorkHandler = void (*)(InterruptWork*);

    // 中断号
    uint64_t interrupt_no;
    // 中断上下文
    cpu_io::TrapContext* data;
    // 时间戳
    uint64_t timestamp;

    WorkHandler handler = nullptr;
  };

  /// 中断工作队列
  using InterruptWorkQueue =
      mpmc_queue::MPMCQueue<InterruptWork, kInterruptQueueCapacity>;

  /// 每个核心的调度数据
  std::array<CpuSchedData, SIMPLEKERNEL_MAX_CORE_COUNT> cpu_schedulers_{};

  /// 全局任务表 (PID -> TCB 映射)
  SpinLock task_table_lock_{"task_table_lock"};
  etl::unordered_map<Pid, etl::unique_ptr<TaskControlBlock>,
                     kernel::config::kMaxTasks,
                     kernel::config::kMaxTasksBuckets>
      task_table_;

  /// 中断线程相关数据保护锁
  SpinLock interrupt_threads_lock_{"interrupt_threads_lock"};
  /// 中断号 -> 中断线程映射
  etl::unordered_map<uint64_t, TaskControlBlock*,
                     kernel::config::kMaxInterruptThreads,
                     kernel::config::kMaxInterruptThreadsBuckets>
      interrupt_threads_;
  /// 中断号 -> 工作队列映射
  etl::unordered_map<uint64_t, InterruptWorkQueue*,
                     kernel::config::kMaxInterruptThreads,
                     kernel::config::kMaxInterruptThreadsBuckets>
      interrupt_work_queues_;

  /// PID 分配器
  std::atomic<size_t> pid_allocator_{1};

  /**
   * @brief 分配新的 PID
   * @return size_t 新的 PID
   */
  size_t AllocatePid();

  /**
   * @brief 负载均衡 (空闲 core 窃取任务)
   */
  void Balance();

  /**
   * @brief 获取当前核心的调度数据
   */
  CpuSchedData& GetCurrentCpuSched() {
    return cpu_schedulers_[cpu_io::GetCurrentCoreId()];
  }

  /**
   * @brief 获取线程组的所有线程
   * @param tgid 线程组 ID
   * @return etl::vector<TaskControlBlock*, kernel::config::kMaxReadyTasks>
   * 线程组中的所有线程
   */
  etl::vector<TaskControlBlock*, kernel::config::kMaxReadyTasks> GetThreadGroup(
      Pid tgid);

  /**
   * @brief 向线程组中的所有线程发送信号
   * @param tgid 线程组 ID
   * @param signal 信号编号
   * @note 暂未实现信号机制，预留接口
   */
  void SignalThreadGroup(Pid tgid, int signal);

  /**
   * @brief 回收僵尸进程资源
   * @param task 要回收的任务 (必须处于 kZombie 状态)
   * @note 释放内核栈、页表、TCB，回收 PID
   */
  void ReapTask(TaskControlBlock* task);

  /**
   * @brief 将孤儿进程过继给 init 进程
   * @param parent 退出的父进程
   * @note 在父进程退出时调用，防止子进程变成僵尸无人回收
   */
  void ReparentChildren(TaskControlBlock* parent);
};

using TaskManagerSingleton = etl::singleton<TaskManager>;

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TASK_MANAGER_HPP_
