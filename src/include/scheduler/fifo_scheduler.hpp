/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_FIFO_SCHEDULER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_FIFO_SCHEDULER_HPP_

#include "scheduler_base.hpp"
#include "sk_list"

/**
 * @brief 先来先服务 (FIFO) 调度器，用于普通任务
 */
class FifoScheduler : public SchedulerBase {
 public:
  void Enqueue(TaskControlBlock* task) override { ready_queue.push_back(task); }

  TaskControlBlock* PickNext() override {
    if (ready_queue.empty()) {
      return nullptr;
    }
    TaskControlBlock* next = ready_queue.front();
    ready_queue.pop_front();
    return next;
  }

  /// @name 构造/析构函数
  /// @{
  FifoScheduler() = default;
  FifoScheduler(const FifoScheduler&) = default;
  FifoScheduler(FifoScheduler&&) = default;
  auto operator=(const FifoScheduler&) -> FifoScheduler& = default;
  auto operator=(FifoScheduler&&) -> FifoScheduler& = default;
  ~FifoScheduler() override = default;
  /// @}

 private:
  sk_std::list<TaskControlBlock*> ready_queue;
};

/**
 * @brief 简单的优先级调度器 (暂用 FIFO 模拟)，用于实时任务
 */
class RtScheduler : public FifoScheduler {};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_FIFO_SCHEDULER_HPP_ */
