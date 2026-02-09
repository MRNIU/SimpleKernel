/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_IDLE_SCHEDULER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_IDLE_SCHEDULER_HPP_

#include "scheduler_base.hpp"
#include "task_control_block.hpp"

/**
 * @brief Idle 调度器
 *
 * Idle 调度器特点：
 * - 只管理 idle 任务，通常只有一个 idle 任务
 * - 当没有其他任务可运行时，调度 idle 任务
 * - 最低优先级，只有在所有其他调度器都为空时才会被选中
 * - O(1) 时间复杂度
 */
class IdleScheduler : public SchedulerBase {
 public:
  /**
   * @brief 构造函数
   */
  IdleScheduler() { name = "Idle"; }

  /**
   * @brief 将 idle 任务加入队列
   * @param task 要加入的任务（通常只有一个 idle 任务）
   */
  void Enqueue(TaskControlBlock* task) override {
    idle_task_ = task;
    stats_.total_enqueues++;
  }

  /**
   * @brief 从队列中移除任务
   * @param task 要移除的任务
   */
  void Dequeue(TaskControlBlock* task) override {
    if (idle_task_ == task) {
      idle_task_ = nullptr;
      stats_.total_dequeues++;
    }
  }

  /**
   * @brief 选择下一个要运行的任务（返回 idle 任务）
   * @return idle 任务，如果没有则返回 nullptr
   */
  TaskControlBlock* PickNext() override {
    if (idle_task_) {
      stats_.total_picks++;
    }
    // 注意：idle 任务不从队列中移除，因为它应该一直保持可用
    return idle_task_;
  }

  /**
   * @brief 获取队列大小
   * @return 队列大小（0 或 1）
   */
  auto GetQueueSize() const -> size_t override { return idle_task_ ? 1 : 0; }

  /**
   * @brief 判断队列是否为空
   * @return 如果没有 idle 任务则返回 true
   */
  auto IsEmpty() const -> bool override { return idle_task_ == nullptr; }

  /**
   * @brief Tick 更新（idle 任务不需要时间片管理）
   * @param current 当前任务
   * @return 始终返回 false（不需要重新调度）
   */
  auto OnTick([[maybe_unused]] TaskControlBlock* current) -> bool override {
    return false;
  }

  /**
   * @brief 时间片耗尽处理（idle 任务不使用时间片）
   * @param task 任务
   * @return 始终返回 false（不需要重新入队）
   */
  auto OnTimeSliceExpired([[maybe_unused]] TaskControlBlock* task)
      -> bool override {
    return false;
  }

  /**
   * @brief 任务被抢占时的处理（idle 任务不需要特殊处理）
   * @param task 被抢占的任务
   */
  void OnPreempted([[maybe_unused]] TaskControlBlock* task) override {
    stats_.total_preemptions++;
    // Idle 任务被抢占时不需要做任何事，它会一直保持在队列中
  }

  /**
   * @brief 任务被调度时的处理（idle 任务不需要特殊处理）
   * @param task 被调度的任务
   */
  void OnScheduled([[maybe_unused]] TaskControlBlock* task) override {
    // Idle 任务被调度时不需要做任何事
  }

  /**
   * @brief 析构函数
   */
  virtual ~IdleScheduler() = default;

 private:
  /// Idle 任务指针（通常只有一个）
  TaskControlBlock* idle_task_ = nullptr;
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_IDLE_SCHEDULER_HPP_ */
