/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_FIFO_SCHEDULER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_FIFO_SCHEDULER_HPP_

#include "scheduler_base.hpp"
#include "sk_list"
#include "sk_priority_queue"
#include "task_control_block.hpp"

/**
 * @brief 先来先服务 (FIFO) 调度器
 *
 * FIFO 调度器特点：
 * - 先进先出，适合对延迟敏感的实时任务
 * - 不可抢占，任务运行直到主动让出 CPU
 * - O(1) 时间复杂度
 */
class FifoScheduler : public SchedulerBase {
 public:
  /**
   * @brief 构造函数
   */
  FifoScheduler() { name = "FIFO"; }

  /**
   * @brief 将任务加入就绪队列尾部
   * @param task 要加入的任务
   */
  void Enqueue(TaskControlBlock* task) override {
    ready_queue.push_back(task);
    stats_.total_enqueues++;
  }

  /**
   * @brief 从就绪队列中移除指定任务
   * @param task 要移除的任务
   */
  void Dequeue(TaskControlBlock* task) override {
    ready_queue.remove(task);
    stats_.total_dequeues++;
  }

  /**
   * @brief 选择下一个要运行的任务（队列头部）
   * @return 下一个任务，如果队列为空则返回 nullptr
   */
  TaskControlBlock* PickNext() override {
    if (ready_queue.empty()) {
      return nullptr;
    }
    TaskControlBlock* next = ready_queue.front();
    ready_queue.pop_front();
    stats_.total_picks++;
    return next;
  }

  /**
   * @brief 获取就绪队列大小
   * @return 队列中的任务数量
   */
  auto GetQueueSize() const -> size_t override { return ready_queue.size(); }

  /**
   * @brief 判断队列是否为空
   * @return 队列为空返回 true，否则返回 false
   */
  auto IsEmpty() const -> bool override { return ready_queue.empty(); }

  /**
   * @brief 任务被抢占时调用
   *
   * FIFO 是不可抢占调度器，但如果外部强制抢占，则需要统计
   *
   * @param task 被抢占的任务
   */
  void OnPreempted([[maybe_unused]] TaskControlBlock* task) override {
    stats_.total_preemptions++;
  }

  /// @name 构造/析构函数
  /// @{
  FifoScheduler(const FifoScheduler&) = default;
  FifoScheduler(FifoScheduler&&) = default;
  auto operator=(const FifoScheduler&) -> FifoScheduler& = default;
  auto operator=(FifoScheduler&&) -> FifoScheduler& = default;
  ~FifoScheduler() override = default;
  /// @}

 private:
  /// 就绪队列 (先进先出)
  sk_std::list<TaskControlBlock*> ready_queue;
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_FIFO_SCHEDULER_HPP_ */
